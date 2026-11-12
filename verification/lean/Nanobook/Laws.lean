/-
Laws of the limit book.
Modify orders and stop orders are not in this model.
The events are limit submits, market submits, and cancels.
-/

import Nanobook.Spec
import Nanobook.Queue

namespace Nanobook

/-- Books reached from `Book.empty` by `apply` on any event list. -/
def reach (events : List Event) : Book :=
  events.foldl (fun book event => (apply book event).1) Book.empty

def Reachable (book : Book) : Prop :=
  ∃ events, reach events = book

theorem Reachable.empty : Reachable Book.empty :=
  ⟨[], rfl⟩

theorem Reachable.apply {book : Book} (h : Reachable book) (event : Event) :
    Reachable (apply book event).1 := by
  rcases h with ⟨events, rfl⟩
  refine ⟨events ++ [event], ?_⟩
  simp [reach, List.foldl_append]

/-- One submit, with the price the matcher uses. -/
structure SubmitView where
  side : Side
  price : Int
  quantity : Nat

def eventSubmit : Event → Option SubmitView
  | .submitLimit side price quantity _ _ _ => some ⟨side, price, quantity⟩
  | .submitMarket side quantity _ _ =>
      some ⟨side, match side with | .buy => i64Max | .sell => i64Min, quantity⟩
  | .cancel _ => none

def queueIds (book : Book) : List Nat :=
  book.bids.flatMap (·.orders) ++ book.asks.flatMap (·.orders)

/-- A queued id is an active order at that level's price. -/
def queuedActive (book : Book) (side : Side) : Prop :=
  ∀ level ∈ levelsOf book side, ∀ id ∈ level.orders,
    ∃ o, findOrder book.orders id = some o ∧
      o.side = side ∧ o.price = level.price ∧
      o.status.isActive = true ∧ 0 < o.remaining

/-- Bids descend. Asks ascend. -/
def bestFirst (side : Side) (levels : List Level) : Prop :=
  match side with
  | .buy => levels.Pairwise (fun a b => a.price > b.price)
  | .sell => levels.Pairwise (fun a b => a.price < b.price)

def noEmptyLevel (levels : List Level) : Prop :=
  ∀ level ∈ levels, level.orders ≠ []

/-- Every active order stands once, on its own side, at its own price. -/
def activeQueuedOnce (book : Book) : Prop :=
  ∀ p ∈ book.orders, p.2.status.isActive = true →
    (queueIds book).count p.2.id = 1 ∧
    ∃ level ∈ levelsOf book p.2.side,
      level.price = p.2.price ∧ p.2.id ∈ level.orders

/-- Reachable books stay sorted, nonempty, and in one-to-one queue order.
Every queued id is one active order at that level. Every active order is queued once. -/
def law1 : Prop :=
  ∀ book, Reachable book →
    bestFirst .buy book.bids ∧
    bestFirst .sell book.asks ∧
    noEmptyLevel book.bids ∧
    noEmptyLevel book.asks ∧
    (queueIds book).Nodup ∧
    queuedActive book .buy ∧
    queuedActive book .sell ∧
    activeQueuedOnce book

/-- The best bid stays strictly below the best ask. -/
def law2 : Prop :=
  ∀ book, Reachable book →
    ∀ bid ask,
      bestPrice book.bids = some bid →
      bestPrice book.asks = some ask →
      bid < ask

def tradeQtySum (trades : List Trade) : Nat :=
  trades.foldl (fun sum trade => sum + trade.quantity) 0

def filledQty (book : Book) (id : Nat) : Nat :=
  match findOrder book.orders id with
  | some o => o.filled
  | none => 0

/-- Remaining quantity of an order that is still active. -/
def restedQty (book : Book) (id : Nat) : Nat :=
  match findOrder book.orders id with
  | some o => if o.status.isActive then o.remaining else 0
  | none => 0

/-- Submitted quantity that neither fills nor rests. -/
def cancelledQty (book : Book) (id submitted : Nat) : Nat :=
  submitted - filledQty book id - restedQty book id

/-- A submit conserves quantity.
Trade quantities sum to the filled quantity, and that quantity does not exceed the submit.
Submitted quantity equals filled plus rested plus cancelled.
Each trade cuts the resting remainder by the trade quantity, and every trade quantity is positive. -/
def law3 : Prop :=
  ∀ book, Reachable book →
    ∀ event view book' trades,
      eventSubmit event = some view →
      apply book event = (book', trades) →
      let id := book.nextOrderId
      tradeQtySum trades = filledQty book' id ∧
      filledQty book' id ≤ view.quantity ∧
      view.quantity =
        filledQty book' id + restedQty book' id + cancelledQty book' id view.quantity ∧
      (∀ t ∈ trades, 0 < t.quantity) ∧
      (∀ t ∈ trades,
        ∃ before after,
          findOrder book.orders t.passiveId = some before ∧
          findOrder book'.orders t.passiveId = some after ∧
          before.remaining = after.remaining + t.quantity)

/-- `id` occurs in `ids` before `last`. -/
def appearsBefore : List Nat → Nat → Nat → Prop
  | [], _, _ => False
  | head :: tail, id, last =>
      (head = id ∧ last ∈ tail) ∨ (head ≠ id ∧ head ≠ last ∧ appearsBefore tail id last)

/-- `id` sits in the queue before the last traded id, and no trade names it. -/
def skippedBeforeLast (queue traded : List Nat) (id : Nat) : Prop :=
  ∃ last, traded.getLast? = some last ∧ appearsBefore queue id last ∧ id ∉ traded

/-- One submit trades in price-time order.
Resting ids trade from the best price, then from the front of that level.
An id skipped before the last trade was removed by self-trade prevention. -/
def law4 : Prop :=
  ∀ book, Reachable book →
    ∀ event view book' trades,
      eventSubmit event = some view →
      apply book event = (book', trades) →
      let queue := (levelsOf book view.side.opposite).flatMap (·.orders)
      let traded := trades.map (·.passiveId)
      List.Sublist traded queue ∧
      ∀ id, skippedBeforeLast queue traded id →
        ∃ before after,
          findOrder book.orders id = some before ∧
          before.status.isActive = true ∧
          findOrder book'.orders id = some after ∧
          after.status = .cancelled ∧
          after.remaining = 0

/-- A trade prints the resting level's price.
That price is at or inside the incoming limit: at most for a buy, at least for a sell.
A market submit uses the extreme price, so the limit check holds for every trade it prints. -/
def law5 : Prop :=
  ∀ book, Reachable book →
    ∀ event view book' trades,
      eventSubmit event = some view →
      apply book event = (book', trades) →
      ∀ t ∈ trades,
        (∃ level, level ∈ levelsOf book view.side.opposite ∧
          t.passiveId ∈ level.orders ∧ t.price = level.price) ∧
        match view.side with
        | .buy => t.price ≤ view.price
        | .sell => t.price ≥ view.price

/-- Extra fuel never changes the match.
The fuel bound is enough for the loop to stop. -/
def law6 : Prop :=
  ∀ book, Reachable book →
    ∀ incoming policy k,
      matchLoop (matchFuel book incoming + k) book incoming policy [] false =
      matchLoop (matchFuel book incoming) book incoming policy [] false

/-- A fill-or-kill submit trades its whole quantity, or it trades nothing.
A rejected fill-or-kill leaves orders and levels unchanged.
Only the id counter and the timestamp counter advance. -/
def law7 : Prop :=
  ∀ book, Reachable book →
    ∀ side price quantity owner policy,
      let result := apply book (.submitLimit side price quantity .fok owner policy)
      tradeQtySum result.2 = quantity ∨
        (result.2 = [] ∧
          result.1.bids = book.bids ∧
          result.1.asks = book.asks ∧
          result.1.orders = book.orders ∧
          result.1.nextTradeId = book.nextTradeId ∧
          result.1.nextOrderId = book.nextOrderId + 1 ∧
          result.1.nextTimestamp = book.nextTimestamp + 1)

/-- Both owners are present and equal. -/
def ownersConflict (incoming resting : Option Nat) : Bool :=
  match incoming, resting with
  | some a, some b => a == b
  | _, _ => false

/-- Self-trade prevention matches the policy in `src/matching.rs`.
`Off` lets the two orders trade.
`CancelNewest` cancels the incoming remainder and leaves the resting order.
`CancelOldest` cancels the resting order and the incoming order continues.
`DecrementAndCancel` cancels the smaller remainder, or the resting order when the remainders are equal.
With any policy other than `Off`, a trade does not have the same present owner on both sides. -/
def law8 : Prop :=
  ∀ book incoming policy stp,
    (match (step book incoming policy stp).trade with
      | none => True
      | some t =>
        policy = .off ∨
          ∀ resting, findOrder book.orders t.passiveId = some resting →
            ownersConflict incoming.owner resting.owner = false) ∧
    ∀ rid resting px,
      stp = false →
      0 < incoming.remaining →
      bestPrice (levelsOf book incoming.side.opposite) = some px →
      pricesCross incoming.side incoming.price px = true →
      frontAt (levelsOf book incoming.side.opposite) px = some rid →
      findOrder book.orders rid = some resting →
      resting.status.isActive = true →
      0 < resting.remaining →
      ownersConflict incoming.owner resting.owner = true →
      let s := step book incoming policy false
      match policy with
      | .off =>
        ∃ t, s.trade = some t ∧ t.price = px ∧ t.passiveId = rid ∧
          t.quantity = min incoming.remaining resting.remaining ∧
          s.go = true ∧ s.stp = false
      | .cancelNewest =>
        s.book = book ∧ s.incoming = stopIncoming incoming ∧ s.stp = true ∧
          s.trade = none ∧ s.go = false
      | .cancelOldest =>
        s.book = cancelOrder book rid ∧ s.incoming = incoming ∧ s.stp = false ∧
          s.trade = none ∧ s.go = true
      | .decrementAndCancel =>
        if incoming.remaining < resting.remaining then
          s.book = book ∧ s.incoming = stopIncoming incoming ∧ s.stp = true ∧
            s.trade = none ∧ s.go = false
        else
          s.book = cancelOrder book rid ∧ s.incoming = incoming ∧ s.stp = false ∧
            s.trade = none ∧ s.go = true

/-- Cancel of `x` changes only order `x` and drops only `x` from its level.
Every other order keeps its record and its place in the queue.
Cancel of a missing or finished order leaves the book unchanged. -/
def law9 : Prop :=
  ∀ book, Reachable book →
    ∀ x,
      (apply book (.cancel x)).2 = [] ∧
      let book' := (apply book (.cancel x)).1
      match findOrder book.orders x with
      | none => book' = book
      | some o =>
        if o.status.isActive = false then
          book' = book
        else
          book'.nextOrderId = book.nextOrderId ∧
          book'.nextTradeId = book.nextTradeId ∧
          book'.nextTimestamp = book.nextTimestamp ∧
          findOrder book'.orders x =
            some { o with remaining := 0, status := .cancelled } ∧
          (∀ y, y ≠ x → findOrder book'.orders y = findOrder book.orders y) ∧
          levelsOf book' o.side = removeAt (levelsOf book o.side) o.price x ∧
          levelsOf book' o.side.opposite = levelsOf book o.side.opposite

/-- Tombstone queues match the plain FIFO lists.
`toPlain` drops every `0`. `abstractsBook` says each side equals those lists, in the same order.
`levelInv` says `tombstone_count` equals the number of zeros.
It says `total_quantity` equals the sum of the live remainders.
It says `is_empty` holds exactly when no live id remains.
For each live order, `position_in_level - popped_from_front` is the index of that same id.
`linked` says each slot and the plain order agree on side, price, remainder, and active status.
`holds` is these three claims together.
`allowed` is one engine step: insert, cancel, `compact_book`, `front`, a full pop, or a partial decrease.
An insert uses a fresh nonzero id and a positive quantity.
A full pop passes the front live order's own remainder.
A partial decrease passes a smaller positive quantity.
`Level.compact` alone does not rewrite positions. `compact_book` does.
The empty book satisfies `holds`.
Each allowed step keeps `holds`.
Every engine sequence is a chain of those steps, so the four queue properties hold after every sequence. -/
def law10 : Prop :=
  holds QueueBook.empty Book.empty ∧
  ∀ q plain step,
    holds q plain →
    allowed q step →
    holds (applyQ step q) (applyPlain step plain)

/-- A submit leaves a record whose quantities match its status.
Filled: the whole submit filled and nothing remains.
New or partly filled: filled plus remaining equals the submit, and some quantity remains.
Cancelled: nothing remains and less than the submit filled. This covers an immediate-or-cancel
remainder and a stop by self-trade prevention.
No record: a rejected fill-or-kill, which prints no trade.
Law 3 only bounds filled plus rested by the submit. This law says where the rest went. -/
def law11 : Prop :=
  ∀ book, Reachable book →
    ∀ event view book' trades,
      eventSubmit event = some view →
      apply book event = (book', trades) →
      match findOrder book'.orders book.nextOrderId with
      | none => trades = []
      | some o =>
        (o.status = .filled → o.filled = view.quantity ∧ o.remaining = 0) ∧
        (o.status.isActive = true → o.filled + o.remaining = view.quantity ∧ 0 < o.remaining) ∧
        (o.status = .cancelled → o.remaining = 0 ∧ o.filled < view.quantity)

end Nanobook
