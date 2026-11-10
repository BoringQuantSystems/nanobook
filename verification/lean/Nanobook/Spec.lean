/-
Limit book as plain lists, and the corpus operations.
-/

namespace Nanobook

inductive Side where
  | buy
  | sell
  deriving BEq, Repr

def Side.opposite : Side → Side
  | .buy => .sell
  | .sell => .buy

inductive TimeInForce where
  | gtc
  | ioc
  | fok
  deriving BEq, Repr

inductive StpPolicy where
  | off
  | cancelNewest
  | cancelOldest
  | decrementAndCancel
  deriving BEq, Repr

inductive OrderStatus where
  | new
  | partiallyFilled
  | filled
  | cancelled
  deriving BEq, Repr

def OrderStatus.isActive : OrderStatus → Bool
  | .new | .partiallyFilled => true
  | .filled | .cancelled => false

/-- Buy at `i64::MAX`. Sell at `i64::MIN`. -/
def i64Max : Int := 9223372036854775807

def i64Min : Int := -9223372036854775808

structure Order where
  id : Nat
  side : Side
  price : Int
  remaining : Nat
  filled : Nat
  timeInForce : TimeInForce
  owner : Option Nat
  status : OrderStatus
  deriving Repr

structure Level where
  price : Int
  orders : List Nat
  deriving Repr

structure Trade where
  id : Nat
  price : Int
  quantity : Nat
  aggressorId : Nat
  passiveId : Nat
  aggressorSide : Side
  timestamp : Nat
  deriving Repr

structure Book where
  bids : List Level
  asks : List Level
  orders : List (Nat × Order)
  nextOrderId : Nat
  nextTradeId : Nat
  nextTimestamp : Nat
  deriving Repr

def Book.empty : Book where
  bids := []
  asks := []
  orders := []
  nextOrderId := 1
  nextTradeId := 1
  nextTimestamp := 1

inductive Event where
  | submitLimit
      (side : Side) (price : Int) (quantity : Nat) (tif : TimeInForce)
      (owner : Option Nat) (policy : StpPolicy)
  | submitMarket
      (side : Side) (quantity : Nat) (owner : Option Nat) (policy : StpPolicy)
  | cancel (orderId : Nat)
  deriving Repr

def findOrder (orders : List (Nat × Order)) (id : Nat) : Option Order :=
  match orders with
  | [] => none
  | (i, o) :: rest => if i == id then some o else findOrder rest id
termination_by orders

def upsert (orders : List (Nat × Order)) (o : Order) : List (Nat × Order) :=
  match orders with
  | [] => [(o.id, o)]
  | (i, h) :: rest =>
    if i == o.id then (o.id, o) :: rest else (i, h) :: upsert rest o
termination_by orders

def writeOrder (book : Book) (o : Order) : Book :=
  { book with orders := upsert book.orders o }

def levelsOf (book : Book) (side : Side) : List Level :=
  match side with
  | .buy => book.bids
  | .sell => book.asks

def setLevels (book : Book) (side : Side) (levels : List Level) : Book :=
  match side with
  | .buy => { book with bids := levels }
  | .sell => { book with asks := levels }

def removeId (ids : List Nat) (id : Nat) : List Nat :=
  match ids with
  | [] => []
  | h :: rest => if h == id then rest else h :: removeId rest id
termination_by ids

def removeAt (levels : List Level) (price : Int) (id : Nat) : List Level :=
  match levels with
  | [] => []
  | h :: rest =>
    if h.price == price then
      let orders := removeId h.orders id
      if orders.isEmpty then rest else { h with orders := orders } :: rest
    else
      h :: removeAt rest price id
termination_by levels

def dropPrice (levels : List Level) (price : Int) : List Level :=
  match levels with
  | [] => []
  | h :: rest => if h.price == price then rest else h :: dropPrice rest price
termination_by levels

/-- Insert `id` on the bid side. Higher prices stay in front. -/
def insertBid (levels : List Level) (price : Int) (id : Nat) : List Level :=
  match levels with
  | [] => [{ price := price, orders := [id] }]
  | h :: rest =>
    if price > h.price then
      { price := price, orders := [id] } :: levels
    else if price == h.price then
      { h with orders := h.orders ++ [id] } :: rest
    else
      h :: insertBid rest price id
termination_by levels

/-- Insert `id` on the ask side. Lower prices stay in front. -/
def insertAsk (levels : List Level) (price : Int) (id : Nat) : List Level :=
  match levels with
  | [] => [{ price := price, orders := [id] }]
  | h :: rest =>
    if price < h.price then
      { price := price, orders := [id] } :: levels
    else if price == h.price then
      { h with orders := h.orders ++ [id] } :: rest
    else
      h :: insertAsk rest price id
termination_by levels

def enqueue (book : Book) (o : Order) : Book :=
  match o.side with
  | .buy => { book with bids := insertBid book.bids o.price o.id }
  | .sell => { book with asks := insertAsk book.asks o.price o.id }

def dequeue (book : Book) (side : Side) (price : Int) (id : Nat) : Book :=
  setLevels book side (removeAt (levelsOf book side) price id)

def dropLevel (book : Book) (side : Side) (price : Int) : Book :=
  setLevels book side (dropPrice (levelsOf book side) price)

/-- Cancel one active order. A finished order stays as it is. -/
def cancelOrder (book : Book) (id : Nat) : Book :=
  match findOrder book.orders id with
  | none => book
  | some o =>
    if !o.status.isActive then
      book
    else
      let o : Order := { o with remaining := 0, status := .cancelled }
      dequeue (writeOrder book o) o.side o.price id

def bestPrice (levels : List Level) : Option Int :=
  match levels with
  | [] => none
  | h :: _ => some h.price

def frontAt (levels : List Level) (price : Int) : Option Nat :=
  match levels with
  | [] => none
  | h :: rest => if h.price == price then h.orders.head? else frontAt rest price
termination_by levels

def pricesCross (side : Side) (incomingPrice restingPrice : Int) : Bool :=
  match side with
  | .buy => decide (incomingPrice ≥ restingPrice)
  | .sell => decide (incomingPrice ≤ restingPrice)

def sumIds (orders : List (Nat × Order)) (ids : List Nat) : Nat :=
  match ids with
  | [] => 0
  | id :: rest =>
    let qty :=
      match findOrder orders id with
      | some o => if o.status.isActive then o.remaining else 0
      | none => 0
    qty + sumIds orders rest
termination_by ids

-- Count every crossing order. Self-trade policy does not reduce this quantity.
def sumCrossing (orders : List (Nat × Order)) (levels : List Level) (side : Side)
    (price : Int) : Nat :=
  match levels with
  | [] => 0
  | h :: rest =>
    if pricesCross side price h.price then
      sumIds orders h.orders + sumCrossing orders rest side price
    else
      0
termination_by levels

def available (book : Book) (side : Side) (price : Int) : Nat :=
  sumCrossing book.orders (levelsOf book side.opposite) side price

def alloc (book : Book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) : Book × Order :=
  let order : Order := {
    id := book.nextOrderId
    side := side
    price := price
    remaining := quantity
    filled := 0
    timeInForce := tif
    owner := owner
    status := .new
  }
  let book := { book with
    nextOrderId := book.nextOrderId + 1
    nextTimestamp := book.nextTimestamp + 1 }
  (book, order)

def fill (o : Order) (qty : Nat) : Order :=
  let remaining := o.remaining - qty
  { o with
    remaining := remaining
    filled := o.filled + qty
    status := if remaining == 0 then .filled else .partiallyFilled }

def emitTrade (book : Book) (price : Int) (qty : Nat) (aggressor passive : Nat)
    (side : Side) : Book × Trade :=
  let trade : Trade := {
    id := book.nextTradeId
    price := price
    quantity := qty
    aggressorId := aggressor
    passiveId := passive
    aggressorSide := side
    timestamp := book.nextTimestamp
  }
  let book := { book with
    nextTradeId := book.nextTradeId + 1
    nextTimestamp := book.nextTimestamp + 1 }
  (book, trade)

def stpConflict (incoming resting : Order) (policy : StpPolicy) : Bool :=
  match incoming.owner, resting.owner, policy with
  | some _, some _, .off => false
  | some a, some b, _ => a == b
  | _, _, _ => false

def stopIncoming (o : Order) : Order :=
  { o with remaining := 0, status := .cancelled }

structure Step where
  book : Book
  incoming : Order
  stp : Bool
  trade : Option Trade
  go : Bool

/-- One match step. `go` is false when the sweep ends. -/
def step (book : Book) (incoming : Order) (policy : StpPolicy) (stp : Bool) : Step :=
  if stp || incoming.remaining == 0 then
    { book, incoming, stp, trade := none, go := false }
  else
    let opp := incoming.side.opposite
    match bestPrice (levelsOf book opp) with
    | none => { book, incoming, stp, trade := none, go := false }
    | some px =>
      if !pricesCross incoming.side incoming.price px then
        { book, incoming, stp, trade := none, go := false }
      else
        match frontAt (levelsOf book opp) px with
        | none =>
          { book := dropLevel book opp px, incoming, stp, trade := none, go := true }
        | some rid =>
          match findOrder book.orders rid with
          | none =>
            { book := dequeue book opp px rid, incoming, stp, trade := none, go := true }
          | some resting =>
            if !resting.status.isActive || resting.remaining == 0 then
              { book := dequeue book opp px rid, incoming, stp, trade := none, go := true }
            else if stpConflict incoming resting policy && policy == .cancelNewest then
              { book, incoming := stopIncoming incoming, stp := true, trade := none, go := false }
            else if stpConflict incoming resting policy && policy == .cancelOldest then
              { book := cancelOrder book rid, incoming, stp, trade := none, go := true }
            else if stpConflict incoming resting policy &&
                policy == .decrementAndCancel &&
                incoming.remaining < resting.remaining then
              { book, incoming := stopIncoming incoming, stp := true, trade := none, go := false }
            else if stpConflict incoming resting policy && policy == .decrementAndCancel then
              { book := cancelOrder book rid, incoming, stp, trade := none, go := true }
            else
              let qty := min incoming.remaining resting.remaining
              let (book, trade) := emitTrade book px qty incoming.id rid incoming.side
              let incoming := fill incoming qty
              let resting := fill resting qty
              let book := writeOrder book resting
              let book :=
                if resting.remaining == 0 then dequeue book opp px rid else book
              { book, incoming, stp := false, trade := some trade, go := true }

def queuedCount (levels : List Level) : Nat :=
  match levels with
  | [] => 0
  | h :: rest => h.orders.length + queuedCount rest
termination_by levels

def levelCount (levels : List Level) : Nat :=
  match levels with
  | [] => 0
  | _ :: rest => levelCount rest + 1
termination_by levels

/-- Incoming remainder, plus queued ids, plus the number of levels. -/
def matchFuel (book : Book) (incoming : Order) : Nat :=
  incoming.remaining + queuedCount book.bids + queuedCount book.asks +
    levelCount book.bids + levelCount book.asks

def matchLoop (fuel : Nat) (book : Book) (incoming : Order) (policy : StpPolicy)
    (rev : List Trade) (stp : Bool) : Book × Order × List Trade × Bool :=
  match fuel with
  | 0 => (book, incoming, rev.reverse, stp)
  | n + 1 =>
    let s := step book incoming policy stp
    let rev := match s.trade with
      | none => rev
      | some t => t :: rev
    if s.go then
      matchLoop n s.book s.incoming policy rev s.stp
    else
      (s.book, s.incoming, rev.reverse, s.stp)
termination_by fuel

def settle (book : Book) (o : Order) (stp : Bool) : Book :=
  if stp then
    writeOrder book { o with status := .cancelled }
  else if o.remaining == 0 then
    writeOrder book { o with status := .filled }
  else if o.timeInForce == .gtc then
    let status := if o.filled > 0 then OrderStatus.partiallyFilled else OrderStatus.new
    enqueue (writeOrder book { o with status := status }) o
  else
    writeOrder book { o with remaining := 0, status := .cancelled }

-- Reject a fill-or-kill order when crossing quantity is short. The id counter still advances.
def submit (book : Book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) (policy : StpPolicy) : Book × List Trade :=
  if tif == .fok && available book side price < quantity then
    let (book, _) := alloc book side price quantity tif owner
    (book, [])
  else
    let (book, incoming) := alloc book side price quantity tif owner
    let fuel := matchFuel book incoming
    let (book, incoming, trades, stp) := matchLoop fuel book incoming policy [] false
    (settle book incoming stp, trades)

def apply (book : Book) (event : Event) : Book × List Trade :=
  match event with
  | .submitLimit side price quantity tif owner policy =>
    submit book side price quantity tif owner policy
  | .submitMarket side quantity owner policy =>
    let price := match side with
      | .buy => i64Max
      | .sell => i64Min
    submit book side price quantity .ioc owner policy
  | .cancel orderId =>
    (cancelOrder book orderId, [])

def replayAux (events : List Event) (book : Book) (rev : List Trade) : List Trade :=
  match events with
  | [] => rev.reverse
  | e :: rest =>
    let (book, trades) := apply book e
    replayAux rest book (trades.reverse ++ rev)
termination_by events

def replay (events : List Event) : List Trade :=
  replayAux events Book.empty []

end Nanobook
