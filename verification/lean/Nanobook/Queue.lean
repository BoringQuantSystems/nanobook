/-
Price queue with tombstones and stored positions.
Plain FIFO lists live in `Spec.lean`.
`0` is a tombstone id.
`Nat` subtraction saturates, as Rust `saturating_sub` does.
Rust `usize` subtraction panics in a debug build on underflow, and it wraps in a release build.
`Nat` saturates on those lines too.
`Nat` addition does not wrap. Rust `usize` addition wraps in a release build.
-/

import Nanobook.Spec

namespace Nanobook

/-- One price queue. Fields match `Level` in `src/level.rs`. -/
structure QueueLevel where
  price : Int
  orders : List Nat
  total_quantity : Nat
  tombstone_count : Nat
  popped_from_front : Nat
  deriving Repr

/-- `Level::new` in `src/level.rs`. -/
def QueueLevel.new (price : Int) : QueueLevel where
  price := price
  orders := []
  total_quantity := 0
  tombstone_count := 0
  popped_from_front := 0

/-- `Level::is_empty` in `src/level.rs`. True when the exact total is zero. -/
def is_empty (level : QueueLevel) : Bool :=
  level.total_quantity == 0

/-- `Level::order_count` in `src/level.rs`. Tombstones are not counted. -/
def order_count (level : QueueLevel) : Nat :=
  level.orders.length - level.tombstone_count

/-- `Level::next_position` in `src/level.rs`. Position of the next pushed id. -/
def next_position (level : QueueLevel) : Nat :=
  level.popped_from_front + level.orders.length

/-- `Level::front_position` in `src/level.rs`. Position of the current front entry. -/
def front_position (level : QueueLevel) : Nat :=
  level.popped_from_front

/-- `Level::push_back` in `src/level.rs`.
Rust panics when `orderId` is 0. The two stores below still run here. -/
def push_back (level : QueueLevel) (orderId quantity : Nat) : QueueLevel :=
  { level with
    orders := level.orders ++ [orderId]
    total_quantity := level.total_quantity + quantity }

/-- Loop of `Level::front` in `src/level.rs`. A leading `0` is popped. -/
def frontLoop (orders : List Nat) (price : Int) (total popped tombstones : Nat) :
    QueueLevel × Option Nat :=
  match orders with
  | [] =>
    ({ price := price, orders := [], total_quantity := total,
       tombstone_count := tombstones, popped_from_front := popped }, none)
  | id :: rest =>
    if id == 0 then
      -- Rust uses `-= 1` on the tombstone count. `Nat` subtraction saturates.
      frontLoop rest price total (popped + 1) (tombstones - 1)
    else
      ({ price := price, orders := orders, total_quantity := total,
         tombstone_count := tombstones, popped_from_front := popped }, some id)
termination_by orders

/-- `Level::front` in `src/level.rs`. Pop leading tombstones. Return the first live id. -/
def front (level : QueueLevel) : QueueLevel × Option Nat :=
  frontLoop level.orders level.price level.total_quantity level.popped_from_front
    level.tombstone_count

/-- Loop of `Level::pop_front` in `src/level.rs`. -/
def popLoop (orders : List Nat) (price : Int) (popped tombstones total quantity : Nat) :
    QueueLevel × Option Nat :=
  match orders with
  | [] =>
    ({ price := price, orders := [], total_quantity := total,
       tombstone_count := tombstones, popped_from_front := popped }, none)
  | id :: rest =>
    let popped := popped + 1
    if id == 0 then
      -- Rust uses `-= 1` on the tombstone count. `Nat` subtraction saturates.
      popLoop rest price popped (tombstones - 1) total quantity
    else
      -- Rust uses `saturating_sub` on the exact total.
      ({ price := price, orders := rest, total_quantity := total - quantity,
         tombstone_count := tombstones, popped_from_front := popped }, some id)
termination_by orders

/-- `Level::pop_front` in `src/level.rs`.
Remove the first live id and subtract `quantity` from the exact total. -/
def pop_front (level : QueueLevel) (quantity : Nat) : QueueLevel × Option Nat :=
  popLoop level.orders level.price level.popped_from_front level.tombstone_count
    level.total_quantity quantity

/-- Read one queue entry. `None` when `index` is past the end. -/
def getAt (ids : List Nat) (index : Nat) : Option Nat :=
  match ids, index with
  | [], _ => none
  | head :: _, 0 => some head
  | _ :: rest, n + 1 => getAt rest n
termination_by ids

/-- Write one queue entry. An index past the end leaves the list as it is. -/
def setAt (ids : List Nat) (index value : Nat) : List Nat :=
  match ids, index with
  | [], _ => []
  | _ :: rest, 0 => value :: rest
  | head :: rest, n + 1 => head :: setAt rest n value
termination_by ids

/-- `Level::mark_tombstone` in `src/level.rs`.
`position - popped_from_front` is the index. A tombstone stays a tombstone. -/
def mark_tombstone (level : QueueLevel) (position quantity : Nat) : QueueLevel :=
  if position < level.popped_from_front then
    level
  else
    let index := position - level.popped_from_front
    match getAt level.orders index with
    | none => level
    | some id =>
      if id == 0 then
        level
      else
        { level with
          orders := setAt level.orders index 0
          -- Rust uses `saturating_sub` on the exact total.
          total_quantity := level.total_quantity - quantity
          tombstone_count := level.tombstone_count + 1 }

/-- `Level::decrease_quantity` in `src/level.rs`.
Rust uses `saturating_sub`. The id stays in the queue. -/
def decrease_quantity (level : QueueLevel) (amount : Nat) : QueueLevel :=
  { level with total_quantity := level.total_quantity - amount }

/-- Drop tombstones. This is `Level::iter` in `src/level.rs`, as a list. -/
def liveIds (ids : List Nat) : List Nat :=
  match ids with
  | [] => []
  | 0 :: rest => liveIds rest
  | id :: rest => id :: liveIds rest
termination_by ids

/-- `Level::compact` in `src/level.rs`.
A zero tombstone count leaves the queue unchanged.
`popped_from_front` stays as it is. -/
def compact (level : QueueLevel) : QueueLevel :=
  if level.tombstone_count == 0 then
    level
  else
    { level with orders := liveIds level.orders, tombstone_count := 0 }

/-- Count of zeros in the queue. -/
def zeroCount (ids : List Nat) : Nat :=
  match ids with
  | [] => 0
  | 0 :: rest => zeroCount rest + 1
  | _ :: rest => zeroCount rest
termination_by ids

/-- One stored order. The queue needs the position, the side, the price, and the remainder. -/
structure QueueOrder where
  side : Side
  price : Int
  remaining : Nat
  position_in_level : Nat
  active : Bool
  deriving Repr

/-- Both sides and the order table. The best-price cache is not stored. -/
structure QueueBook where
  bids : List QueueLevel
  asks : List QueueLevel
  orders : List (Nat × QueueOrder)
  deriving Repr

/-- Empty queue book. -/
def QueueBook.empty : QueueBook where
  bids := []
  asks := []
  orders := []

def queueLevels (q : QueueBook) (side : Side) : List QueueLevel :=
  match side with
  | .buy => q.bids
  | .sell => q.asks

def setQueueLevels (q : QueueBook) (side : Side) (levels : List QueueLevel) : QueueBook :=
  match side with
  | .buy => { q with bids := levels }
  | .sell => { q with asks := levels }

def findSlot (orders : List (Nat × QueueOrder)) (id : Nat) : Option QueueOrder :=
  match orders with
  | [] => none
  | (i, o) :: rest => if i == id then some o else findSlot rest id
termination_by orders

def upsertSlot (orders : List (Nat × QueueOrder)) (id : Nat) (o : QueueOrder) :
    List (Nat × QueueOrder) :=
  match orders with
  | [] => [(id, o)]
  | (i, h) :: rest =>
    if i == id then (id, o) :: rest else (i, h) :: upsertSlot rest id o
termination_by orders

def writeSlot (q : QueueBook) (id : Nat) (o : QueueOrder) : QueueBook :=
  { q with orders := upsertSlot q.orders id o }

def remainOf (orders : List (Nat × QueueOrder)) (id : Nat) : Nat :=
  match findSlot orders id with
  | some o => o.remaining
  | none => 0

def setPos (orders : List (Nat × QueueOrder)) (id position : Nat) :
    List (Nat × QueueOrder) :=
  match orders with
  | [] => []
  | (i, s) :: rest =>
    if i == id then
      (i, { s with position_in_level := position }) :: rest
    else
      (i, s) :: setPos rest id position
termination_by orders

def slotKeys (orders : List (Nat × QueueOrder)) : List Nat :=
  match orders with
  | [] => []
  | (id, _) :: rest => id :: slotKeys rest
termination_by orders

/-- True when `price` comes before `here` on `side`.
A bid comes before a lower price. An ask comes before a higher price.
`insertBid` and `insertAsk` in `Spec.lean` use the same order. -/
def comesFirst (side : Side) (price here : Int) : Bool :=
  match side with
  | .buy => decide (price > here)
  | .sell => decide (price < here)

/-- `PriceLevels::insert_order` in `src/price_levels.rs`.
Return the stored position.
Rust keeps levels in a `BTreeMap`. This list stays in the `Spec.lean` order.
`side` selects that order. -/
def insert_order (side : Side) (levels : List QueueLevel) (price : Int)
    (orderId quantity : Nat) : List QueueLevel × Nat :=
  match levels with
  | [] =>
    let level := QueueLevel.new price
    ([push_back level orderId quantity], next_position level)
  | level :: rest =>
    if comesFirst side price level.price then
      let fresh := QueueLevel.new price
      ((push_back fresh orderId quantity) :: levels, next_position fresh)
    else if price == level.price then
      ((push_back level orderId quantity) :: rest, next_position level)
    else
      let (restLevels, position) := insert_order side rest price orderId quantity
      (level :: restLevels, position)
termination_by levels

/-- First live id in one queue. -/
def firstLive (ids : List Nat) : Option Nat :=
  match ids with
  | [] => none
  | 0 :: rest => firstLive rest
  | id :: _ => some id
termination_by ids

/-- Raw id list at `price`, tombstones included. Empty when the price is missing. -/
def ordersAt (levels : List QueueLevel) (price : Int) : List Nat :=
  match levels with
  | [] => []
  | level :: rest =>
    if level.price == price then level.orders else ordersAt rest price
termination_by levels

def firstLiveAt (levels : List QueueLevel) (price : Int) : Option Nat :=
  firstLive (ordersAt levels price)

/-- `PriceLevels::mark_tombstone` in `src/price_levels.rs`.
An empty level is removed, as `remove_level` does there. -/
def mark_at (levels : List QueueLevel) (price : Int) (position quantity : Nat) :
    List QueueLevel :=
  match levels with
  | [] => []
  | level :: rest =>
    if level.price == price then
      let level' := mark_tombstone level position quantity
      if is_empty level' then rest else level' :: rest
    else
      level :: mark_at rest price position quantity
termination_by levels

/-- `OrderBook::add_order` in `src/book.rs`.
Store the position that `insert_order` returns.
Rust panics when the id is already present. This write replaces that slot. -/
def add_order (q : QueueBook) (side : Side) (price : Int) (id quantity : Nat) : QueueBook :=
  let (levels, position) := insert_order side (queueLevels q side) price id quantity
  let q := setQueueLevels q side levels
  writeSlot q id {
    side := side
    price := price
    remaining := quantity
    position_in_level := position
    active := true
  }

/-- `OrderBook::cancel_order` in `src/book.rs`.
A missing or finished order is left as it is.
The stored position and the old remainder go to `mark_tombstone`. -/
def cancel_order (q : QueueBook) (id : Nat) : QueueBook × Option Nat :=
  match findSlot q.orders id with
  | none => (q, none)
  | some s =>
    if s.active == false then
      (q, none)
    else
      let position := s.position_in_level
      let remaining := s.remaining
      let side := s.side
      let price := s.price
      let q := writeSlot q id { s with remaining := 0, active := false }
      let levels := mark_at (queueLevels q side) price position remaining
      (setQueueLevels q side levels, some remaining)

/-- `PriceLevels::compact` in `src/price_levels.rs`. -/
def compactLevels (levels : List QueueLevel) : List QueueLevel :=
  match levels with
  | [] => []
  | level :: rest => compact level :: compactLevels rest
termination_by levels

/-- Rewrite stored positions after `Level::compact`, as `OrderBook::compact` does.
`offset` counts live ids. `front_position + offset` is the new position. -/
def rewriteLiveFrom (level : QueueLevel) (ids : List Nat) (offset : Nat)
    (orders : List (Nat × QueueOrder)) : List (Nat × QueueOrder) :=
  match ids with
  | [] => orders
  | id :: rest =>
    rewriteLiveFrom level rest (offset + 1)
      (setPos orders id (front_position level + offset))
termination_by ids

def rewriteLive (level : QueueLevel) (orders : List (Nat × QueueOrder)) :
    List (Nat × QueueOrder) :=
  rewriteLiveFrom level (liveIds level.orders) 0 orders

def rewriteLevels (levels : List QueueLevel) (orders : List (Nat × QueueOrder)) :
    List (Nat × QueueOrder) :=
  match levels with
  | [] => orders
  | level :: rest => rewriteLevels rest (rewriteLive level orders)
termination_by levels

/-- `OrderBook::compact` in `src/book.rs`.
Compact each level, then rewrite every live order's stored position. -/
def compact_book (q : QueueBook) : QueueBook :=
  let bids := compactLevels q.bids
  let asks := compactLevels q.asks
  let orders := rewriteLevels asks (rewriteLevels bids q.orders)
  { bids := bids, asks := asks, orders := orders }

/-- `Level::front` on the level at `price`. `match_at_price` in `src/matching.rs` calls it.
A missing price leaves the side unchanged. -/
def front_at (levels : List QueueLevel) (price : Int) : List QueueLevel × Option Nat :=
  match levels with
  | [] => ([], none)
  | level :: rest =>
    if level.price == price then
      let (level', id?) := front level
      (level' :: rest, id?)
    else
      let (rest', id?) := front_at rest price
      (level :: rest', id?)
termination_by levels

/-- Full-fill branch of `match_at_price` in `src/matching.rs`.
`pop_front`, then `remove_level` when the level is empty. -/
def pop_at (levels : List QueueLevel) (price : Int) (quantity : Nat) :
    List QueueLevel × Option Nat :=
  match levels with
  | [] => ([], none)
  | level :: rest =>
    if level.price == price then
      let (level', id?) := pop_front level quantity
      if is_empty level' then
        (rest, id?)
      else
        (level' :: rest, id?)
    else
      let (rest', id?) := pop_at rest price quantity
      (level :: rest', id?)
termination_by levels

/-- Partial-fill branch of `match_at_price` in `src/matching.rs`. -/
def decrease_at (levels : List QueueLevel) (price : Int) (amount : Nat) : List QueueLevel :=
  match levels with
  | [] => []
  | level :: rest =>
    if level.price == price then
      (decrease_quantity level amount) :: rest
    else
      level :: decrease_at rest price amount
termination_by levels

/-- Full fill of the front live order.
Rust fills that order, then calls `pop_front` with its old remainder.
The slot remainder is set to zero here. `pop_front` subtracts `qty` on the level. -/
def pop_front_book (q : QueueBook) (side : Side) (price : Int) (qty : Nat) : QueueBook :=
  let id? := firstLiveAt (queueLevels q side) price
  let q :=
    match id? with
    | none => q
    | some id =>
      match findSlot q.orders id with
      | none => q
      | some s => writeSlot q id { s with remaining := 0, active := false }
  let (levels, _) := pop_at (queueLevels q side) price qty
  setQueueLevels q side levels

/-- Partial fill of the front live order.
Rust calls `Order::fill`, then `decrease_quantity`.
`Order::fill` panics when `amount` exceeds the remainder. `Nat` subtraction saturates. -/
def decrease_book (q : QueueBook) (side : Side) (price : Int) (amount : Nat) : QueueBook :=
  -- `match_at_price` calls `Level::front` before every fill, which pops leading
  -- tombstones and advances `popped_from_front`; apply it first, as Rust does.
  let q := setQueueLevels q side (front_at (queueLevels q side) price).1
  match firstLiveAt (queueLevels q side) price with
  | none => q
  | some id =>
    let q :=
      match findSlot q.orders id with
      | none => q
      | some s => writeSlot q id { s with remaining := s.remaining - amount }
    setQueueLevels q side (decrease_at (queueLevels q side) price amount)

/-- Sum of remainders of live ids. A tombstone adds zero. -/
def liveSum (ids : List Nat) (orders : List (Nat × QueueOrder)) : Nat :=
  match ids with
  | [] => 0
  | 0 :: rest => liveSum rest orders
  | id :: rest => remainOf orders id + liveSum rest orders
termination_by ids

/-- One level agrees with its slots.
`tombstone_count` equals the number of zeros.
`total_quantity` equals the sum of the live remainders.
`is_empty` holds exactly when no live id remains.
For each live id, `position_in_level - popped_from_front` indexes that same id. -/
def levelInv (level : QueueLevel) (orders : List (Nat × QueueOrder)) (side : Side) : Prop :=
  level.tombstone_count = zeroCount level.orders ∧
  level.total_quantity = liveSum level.orders orders ∧
  (is_empty level = true ↔ liveIds level.orders = []) ∧
  ∀ id, id ∈ liveIds level.orders →
    ∃ s, findSlot orders id = some s ∧
      s.active = true ∧
      0 < s.remaining ∧
      s.side = side ∧
      s.price = level.price ∧
      level.popped_from_front ≤ s.position_in_level ∧
      getAt level.orders (s.position_in_level - level.popped_from_front) = some id

def concatLive (levels : List QueueLevel) : List Nat :=
  match levels with
  | [] => []
  | level :: rest => liveIds level.orders ++ concatLive rest
termination_by levels

/-- Queue bookkeeping invariant.
Levels are sorted best first with one level per price: bids strictly descending, asks
strictly ascending, as `BTreeMap` keys are. Without this, a book the engine never builds
(two levels at one price) breaks the active-slot clause after an insert.
Every level has a live id, because Rust drops a level when `is_empty` holds.
Each id has one slot. Each active slot stands in its own level. -/
def bookInv (q : QueueBook) : Prop :=
  q.bids.Pairwise (fun a b => a.price > b.price) ∧
  q.asks.Pairwise (fun a b => a.price < b.price) ∧
  (∀ level ∈ q.bids, levelInv level q.orders .buy) ∧
  (∀ level ∈ q.asks, levelInv level q.orders .sell) ∧
  (∀ level ∈ q.bids, liveIds level.orders ≠ []) ∧
  (∀ level ∈ q.asks, liveIds level.orders ≠ []) ∧
  (concatLive q.bids ++ concatLive q.asks).Nodup ∧
  (slotKeys q.orders).Nodup ∧
  (∀ id s, findSlot q.orders id = some s → s.active = true →
    id ∈ liveIds (ordersAt (queueLevels q s.side) s.price))

/-- Plain level for one queue. Tombstones are dropped. -/
def toPlain (level : QueueLevel) : Level :=
  { price := level.price, orders := liveIds level.orders }

def toPlainLevels (levels : List QueueLevel) : List Level :=
  match levels with
  | [] => []
  | level :: rest => toPlain level :: toPlainLevels rest
termination_by levels

/-- Live ids equal the plain FIFO lists, in the same order. -/
def abstractsBook (q : QueueBook) (plain : Book) : Prop :=
  toPlainLevels q.bids = plain.bids ∧
  toPlainLevels q.asks = plain.asks

/-- Slot and plain order agree on the fields the queue reads. -/
def slotAgrees (s : QueueOrder) (o : Order) : Prop :=
  s.side = o.side ∧
  s.price = o.price ∧
  s.remaining = o.remaining ∧
  s.active = o.status.isActive

/-- The order table matches `Book.orders` in `Spec.lean` on those fields. -/
def linked (q : QueueBook) (plain : Book) : Prop :=
  ∀ id,
    match findSlot q.orders id, findOrder plain.orders id with
    | none, none => True
    | some s, some o => o.id = id ∧ slotAgrees s o
    | _, _ => False

/-- Abstraction plus the queue invariant. -/
def holds (q : QueueBook) (plain : Book) : Prop :=
  abstractsBook q plain ∧ bookInv q ∧ linked q plain

/-- One queue step the engine can take. -/
inductive QStep where
  | insert (side : Side) (price : Int) (id qty : Nat)
  | cancel (id : Nat)
  | compact
  | front (side : Side) (price : Int)
  | popFront (side : Side) (price : Int) (qty : Nat)
  | decrease (side : Side) (price : Int) (amount : Nat)

/-- Steps the engine takes.
Insert uses a fresh nonzero id and a positive quantity.
A full pop passes the front live order's own remainder.
A partial decrease passes a smaller positive quantity.
`front`, cancel, and compact have no further guard.
`Level::remove` is not a step. The engine calls `mark_tombstone`.
The orphan `pop_front(0)` in `match_at_price` is not a step. -/
def allowed (q : QueueBook) : QStep → Prop
  | .insert _ _ id qty => id ≠ 0 ∧ 0 < qty ∧ findSlot q.orders id = none
  | .cancel _ => True
  | .compact => True
  | .front _ _ => True
  | .popFront side price qty =>
    match firstLiveAt (queueLevels q side) price with
    | none => False
    | some id => remainOf q.orders id = qty ∧ 0 < qty
  | .decrease side price amount =>
    match firstLiveAt (queueLevels q side) price with
    | none => False
    | some id => 0 < amount ∧ amount < remainOf q.orders id

/-- Run one step on the queue book. -/
def applyQ (step : QStep) (q : QueueBook) : QueueBook :=
  match step with
  | .insert side price id qty => add_order q side price id qty
  | .cancel id => (cancel_order q id).1
  | .compact => compact_book q
  | .front side price =>
    let (levels, _) := front_at (queueLevels q side) price
    setQueueLevels q side levels
  | .popFront side price qty => pop_front_book q side price qty
  | .decrease side price amount => decrease_book q side price amount

/-- Run the same step on the plain book in `Spec.lean`.
`front` and `compact` do not change a plain list.
Insert does not advance the id counter or the timestamp counter. -/
def applyPlain (step : QStep) (plain : Book) : Book :=
  match step with
  | .insert side price id qty =>
    let o : Order := {
      id := id
      side := side
      price := price
      remaining := qty
      filled := 0
      timeInForce := .gtc
      owner := none
      status := .new
    }
    writeOrder (enqueue plain o) o
  | .cancel id => cancelOrder plain id
  | .compact => plain
  | .front _ _ => plain
  | .popFront side price qty =>
    match frontAt (levelsOf plain side) price with
    | none => plain
    | some id =>
      match findOrder plain.orders id with
      | none => dequeue plain side price id
      | some o =>
        let o := fill o qty
        let plain := writeOrder plain o
        if o.remaining == 0 then dequeue plain side price id else plain
  | .decrease side price amount =>
    match frontAt (levelsOf plain side) price with
    | none => plain
    | some id =>
      match findOrder plain.orders id with
      | none => plain
      | some o => writeOrder plain (fill o amount)

/-
Checks below use the numbers from the `Level` tests in `src/level.rs`.
-/

private def three : QueueLevel :=
  push_back (push_back (push_back (QueueLevel.new 100) 1 100) 2 200) 3 150

#guard order_count three == 3 && three.total_quantity == 450 && (front three).2 == some 1

#guard (pop_front three 100).2 == some 1 && (pop_front three 100).1.total_quantity == 350

private def markedMiddle : QueueLevel :=
  mark_tombstone three 1 200

#guard order_count markedMiddle == 2 && markedMiddle.total_quantity == 250 &&
  markedMiddle.tombstone_count == 1 && liveIds markedMiddle.orders == [1, 3]

private def markedEnds : QueueLevel :=
  mark_tombstone (mark_tombstone three 0 100) 2 150

#guard markedEnds.orders.length == 3 && markedEnds.tombstone_count == 2 &&
  (compact markedEnds).orders == [2] && (compact markedEnds).tombstone_count == 0 &&
  (compact markedEnds).popped_from_front == 0

private def tombFront : QueueLevel :=
  mark_tombstone (push_back (push_back (QueueLevel.new 100) 1 100) 2 200) 0 100

#guard (front tombFront).2 == some 2 && (front tombFront).1.orders == [2] &&
  (front tombFront).1.tombstone_count == 0 && (front tombFront).1.popped_from_front == 1

#guard (decrease_quantity (push_back (push_back (QueueLevel.new 100) 1 100) 2 200) 30).total_quantity ==
  270

#guard (decrease_quantity (push_back (QueueLevel.new 100) 1 100) 200).total_quantity == 0

#guard (pop_front (push_back (decrease_quantity (push_back (QueueLevel.new 100) 1 100) 200) 2 50) 100).1.total_quantity ==
  0

private def rested : QueueBook :=
  add_order (add_order (add_order QueueBook.empty .buy 10 1 5) .buy 10 2 6) .buy 10 3 7

private def afterPop : QueueBook :=
  pop_front_book rested .buy 10 5

#guard match afterPop.bids, findSlot afterPop.orders 3 with
  | [level], some s =>
    level.orders == [2, 3] && level.popped_from_front == 1 && level.total_quantity == 13 &&
      s.position_in_level == 2 && s.active == true
  | _, _ => false

private def cancelAfterFill : QueueBook :=
  (cancel_order afterPop 3).1

#guard match cancelAfterFill.bids with
  | [level] => liveIds level.orders == [2]
  | _ => false

private def hole : QueueBook :=
  (cancel_order rested 2).1

private def holeCompact : QueueBook :=
  compact_book hole

#guard match holeCompact.bids, findSlot holeCompact.orders 3 with
  | [level], some s =>
    level.orders == [1, 3] && level.tombstone_count == 0 && level.popped_from_front == 0 &&
      s.position_in_level == 1 &&
      getAt level.orders (s.position_in_level - level.popped_from_front) == some 3
  | _, _ => false

#guard match findSlot holeCompact.orders 1 with
  | some s => s.position_in_level == 0
  | none => false

#guard match (add_order (add_order QueueBook.empty .sell 10 1 1) .sell 9 2 1).asks with
  | [a, b] => a.price == 9 && a.orders == [2] && b.price == 10 && b.orders == [1]
  | _ => false

#guard match (add_order (add_order QueueBook.empty .buy 10 1 1) .buy 11 2 1).bids with
  | [a, b] => a.price == 11 && a.orders == [2] && b.price == 10 && b.orders == [1]
  | _ => false

#guard match (pop_front_book (add_order QueueBook.empty .buy 10 1 5) .buy 10 5).bids with
  | [] => true
  | _ => false

-- A partial fill first pops leading tombstones, as `match_at_price` calls
-- `Level::front` before every fill: `[0, 2, 3]` becomes `[2, 3]`, popped 1.
#guard
  let q := add_order QueueBook.empty .sell 100 1 5
  let q := add_order q .sell 100 2 5
  let q := add_order q .sell 100 3 5
  let q := (cancel_order q 1).1
  let q := decrease_book q .sell 100 2
  match queueLevels q .sell with
  | [level] => level.orders == [2, 3] && level.popped_from_front == 1
  | _ => false

end Nanobook
