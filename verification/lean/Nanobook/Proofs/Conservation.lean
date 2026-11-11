/-
Quantity and price through one submit.
-/

import Nanobook.Proofs.WellFormed

namespace Nanobook

open List

theorem tradeQtySum_bind (ts : List Trade) (q : Nat) :
    ts.foldl (fun sum t => sum + t.quantity) q =
      q + ts.foldl (fun sum t => sum + t.quantity) 0 := by
  induction ts generalizing q with
  | nil => simp
  | cons t ts ih =>
    simp only [List.foldl]
    rw [ih]
    rw [ih (0 + t.quantity)]
    simp only [Nat.zero_add]
    omega

theorem tradeQtySum_cons (t : Trade) (ts : List Trade) :
    tradeQtySum (t :: ts) = tradeQtySum ts + t.quantity := by
  simp only [tradeQtySum, List.foldl, Nat.zero_add]
  rw [tradeQtySum_bind]
  omega

theorem tradeQtySum_append (a b : List Trade) :
    tradeQtySum (a ++ b) = tradeQtySum a + tradeQtySum b := by
  induction a with
  | nil => simp [tradeQtySum]
  | cons t ts ih =>
    rw [List.cons_append, tradeQtySum_cons, ih, tradeQtySum_cons]
    omega

theorem tradeQtySum_reverse (ts : List Trade) :
    tradeQtySum ts.reverse = tradeQtySum ts := by
  induction ts with
  | nil => simp
  | cons t ts ih =>
    rw [List.reverse_cons, tradeQtySum_append, ih]
    have hone : tradeQtySum [t] = t.quantity := by simp [tradeQtySum, List.foldl]
    rw [hone, tradeQtySum_cons]

theorem nat_split (q f r : Nat) (h : f + r ≤ q) :
    q = f + r + (q - f - r) := by
  omega

theorem cross_buy {limit px : Int} (h : pricesCross .buy limit px = true) : px ≤ limit := by
  simp only [pricesCross, decide_eq_true_eq] at h
  exact h

theorem cross_sell {limit px : Int} (h : pricesCross .sell limit px = true) : limit ≤ px := by
  simp only [pricesCross, decide_eq_true_eq] at h
  exact h

theorem findOrder_below {orders : List (Nat × Order)} {id : Nat}
    (h : ∀ p ∈ orders, p.1 < id) : findOrder orders id = none := by
  induction orders with
  | nil => simp [findOrder]
  | cons entry rest ih =>
    rcases entry with ⟨i, o⟩
    have hi : i ≠ id := Nat.ne_of_lt (h (i, o) mem_cons_self)
    have hbeq : (i == id) = false := by
      cases hb : (i == id)
      · rfl
      · exact absurd (by simpa using hb) hi
    simp [findOrder, hbeq, ih (fun p hp => h p (mem_cons_of_mem _ hp))]

theorem orders_enqueue (book : Book) (o : Order) :
    (enqueue book o).orders = book.orders := by
  cases h : o.side <;> simp [enqueue, h]

/-- A trade price is at or inside the incoming limit. -/
def withinLimit (side : Side) (limit price : Int) : Prop :=
  match side with
  | .buy => price ≤ limit
  | .sell => limit ≤ price

theorem levels_write (book : Book) (o : Order) (side : Side) :
    levelsOf (writeOrder book o) side = levelsOf book side := by
  cases side <;> simp [writeOrder, levelsOf]

theorem levels_emit (book : Book) (price : Int) (qty aggressor passive : Nat) (s side : Side) :
    levelsOf (emitTrade book price qty aggressor passive s).1 side = levelsOf book side := by
  cases side <;> simp [emitTrade, levelsOf]

theorem levels_dequeue (book : Book) (side : Side) (px : Int) (id : Nat) :
    levelsOf (dequeue book side px id) side = removeAt (levelsOf book side) px id := by
  cases side <;> simp [dequeue, setLevels, levelsOf]

/-- Trades agree with the incoming fill, and each cut is still on the book. -/
structure MatchAcc (src : Book) (budget : Nat) (side : Side) (limit : Int)
    (book : Book) (incoming : Order) (trades : List Trade) (stp : Bool) : Prop where
  inv : BookInv book
  ident : incoming.id = src.nextOrderId
  sideEq : incoming.side = side
  limitEq : incoming.price = limit
  sumEq : tradeQtySum trades = incoming.filled
  spent : incoming.filled + incoming.remaining ≤ budget
  live : stp = false → incoming.filled + incoming.remaining = budget
  pos : ∀ t ∈ trades, 0 < t.quantity
  cut : ∀ t ∈ trades, ∃ before after,
    findOrder src.orders t.passiveId = some before ∧
    findOrder book.orders t.passiveId = some after ∧
    before.remaining = after.remaining + t.quantity
  atLevel : ∀ t ∈ trades, ∃ level, level ∈ levelsOf src side.opposite ∧
    t.passiveId ∈ level.orders ∧ t.price = level.price
  worse : ∀ t ∈ trades, withinLimit side limit t.price
  originQueue : ∀ level ∈ levelsOf book side.opposite, ∀ id ∈ level.orders,
    ∃ level0, level0 ∈ levelsOf src side.opposite ∧
      level0.price = level.price ∧ id ∈ level0.orders
  freshQueue : stp = false → incoming.remaining = 0 ∨
    ∀ level ∈ levelsOf book side.opposite, ∀ id ∈ level.orders,
      findOrder book.orders id = findOrder src.orders id
  below : ∀ p ∈ book.orders, p.1 < incoming.id
  nextBound : incoming.id < book.nextOrderId

theorem matchAcc_reverse
    (h : MatchAcc src budget side limit book incoming trades stp) :
    MatchAcc src budget side limit book incoming trades.reverse stp where
  inv := h.inv
  ident := h.ident
  sideEq := h.sideEq
  limitEq := h.limitEq
  sumEq := by rw [tradeQtySum_reverse]; exact h.sumEq
  spent := h.spent
  live := h.live
  pos := fun t ht => h.pos t (List.mem_reverse.mp ht)
  cut := fun t ht => h.cut t (List.mem_reverse.mp ht)
  atLevel := fun t ht => h.atLevel t (List.mem_reverse.mp ht)
  worse := fun t ht => h.worse t (List.mem_reverse.mp ht)
  originQueue := h.originQueue
  freshQueue := h.freshQueue
  below := h.below
  nextBound := h.nextBound

theorem not_passive
    {src book : Book} {trades : List Trade} {rid : Nat}
    (hpos : ∀ t ∈ trades, 0 < t.quantity)
    (hcut : ∀ t ∈ trades, ∃ before after,
      findOrder src.orders t.passiveId = some before ∧
      findOrder book.orders t.passiveId = some after ∧
      before.remaining = after.remaining + t.quantity)
    (hsame : findOrder book.orders rid = findOrder src.orders rid) :
    ∀ t ∈ trades, t.passiveId ≠ rid := by
  intro t ht heq
  rcases hcut t ht with ⟨before, after, hb, ha, hq⟩
  have ha' : findOrder book.orders rid = some after := by simpa [heq] using ha
  have hb' : findOrder src.orders rid = some before := by simpa [heq] using hb
  rw [hsame] at ha'
  rw [hb'] at ha'
  cases ha'
  have := hpos t ht
  omega

theorem origin_removeAt
    {src book book' : Book} {side : Side} {px : Int} {id : Nat}
    (horigin : ∀ level ∈ levelsOf book side, ∀ i ∈ level.orders,
      ∃ level0, level0 ∈ levelsOf src side ∧ level0.price = level.price ∧ i ∈ level0.orders)
    (hlevels : levelsOf book' side = removeAt (levelsOf book side) px id) :
    ∀ level ∈ levelsOf book' side, ∀ i ∈ level.orders,
      ∃ level0, level0 ∈ levelsOf src side ∧ level0.price = level.price ∧ i ∈ level0.orders := by
  intro level hl i hi
  have hl' : level ∈ removeAt (levelsOf book side) px id := by simpa [hlevels] using hl
  rcases mem_of_removeAt hl' hi with ⟨old, hold, hprice, hiold⟩
  rcases horigin old hold i hiold with ⟨level0, hm, hp, hi0⟩
  exact ⟨level0, hm, hp.trans hprice, hi0⟩

theorem fresh_after_edit
    {src book book' : Book} {side : Side} {px : Int} {rid : Nat} {o' : Order}
    {level0 : Level}
    (hinv : BookInv book)
    (hfresh : ∀ level ∈ levelsOf book side, ∀ i ∈ level.orders,
      findOrder book.orders i = findOrder src.orders i)
    (hlev : level0 ∈ levelsOf book side) (hmem : rid ∈ level0.orders) (hpx : level0.price = px)
    (hid : o'.id = rid)
    (horders : book'.orders = upsert book.orders o')
    (hlevels : levelsOf book' side = removeAt (levelsOf book side) px rid) :
    ∀ level ∈ levelsOf book' side, ∀ i ∈ level.orders,
      findOrder book'.orders i = findOrder src.orders i := by
  have hgone : rid ∉ (removeAt (levelsOf book side) px rid).flatMap (·.orders) := by
    simpa [hpx] using
      not_mem_remove_level (best_price_ne (side_best hinv side))
        (side_flat_nodup hinv side) hlev hmem
  intro level hl i hi
  have hl' : level ∈ removeAt (levelsOf book side) px rid := by simpa [hlevels] using hl
  rcases mem_of_removeAt hl' hi with ⟨old, hold, _, hiold⟩
  have hine : i ≠ rid := by
    intro heq
    subst heq
    exact hgone (mem_flatMap.mpr ⟨level, hl', hi⟩)
  have hneO : ¬ i = o'.id := by
    rw [hid]
    exact hine
  rw [horders, findOrder_upsert, ite_eq_right hneO]
  exact hfresh old hold i hiold

theorem matchAcc_stop
    (h : MatchAcc src budget side limit book incoming trades false) :
    MatchAcc src budget side limit book (stopIncoming incoming) trades true where
  inv := h.inv
  ident := by simpa [stopIncoming] using h.ident
  sideEq := by simpa [stopIncoming] using h.sideEq
  limitEq := by simpa [stopIncoming] using h.limitEq
  sumEq := by simpa [stopIncoming] using h.sumEq
  spent := by
    have hlive := h.live rfl
    simp only [stopIncoming]
    omega
  live := by
    intro hfalse
    cases hfalse
  pos := h.pos
  cut := h.cut
  atLevel := h.atLevel
  worse := h.worse
  originQueue := h.originQueue
  freshQueue := by
    intro hfalse
    cases hfalse
  below := by simpa [stopIncoming] using h.below
  nextBound := by simpa [stopIncoming] using h.nextBound

theorem matchAcc_cancel
    (h : MatchAcc src budget side limit book incoming trades false)
    {px : Int} {rid n : Nat} {rest : List Nat} {tail : List Level} {resting : Order}
    (hsucc : incoming.remaining = Nat.succ n)
    (hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hfind : findOrder book.orders rid = some resting)
    (hsideR : resting.side = incoming.side.opposite)
    (hpxR : resting.price = px)
    (hact : resting.status.isActive = true) :
    MatchAcc src budget side limit (cancelOrder book rid) incoming trades false := by
  have hopp : incoming.side.opposite = side.opposite := by rw [h.sideEq]
  have hshapeS : levelsOf book side.opposite =
      { price := px, orders := rid :: rest } :: tail := by
    rw [← hopp]
    exact hshape
  have hidr : resting.id = rid := findOrder_id h.inv.keys hfind
  let o' : Order := { resting with remaining := 0, status := .cancelled }
  have hc := cancelOrder_active hfind hact
  have horders : (cancelOrder book rid).orders = upsert book.orders o' := by
    rw [hc]
    simp [orders_dequeue, writeOrder, o']
  have hlevels : levelsOf (cancelOrder book rid) side.opposite =
      removeAt (levelsOf book side.opposite) px rid := by
    have hc' : cancelOrder book rid =
        dequeue (writeOrder book o') side.opposite px rid := by
      simpa [o', hsideR, hpxR, hopp] using hc
    rw [hc', levels_dequeue, levels_write]
  have hfreshAll : ∀ level ∈ levelsOf book side.opposite, ∀ id ∈ level.orders,
      findOrder book.orders id = findOrder src.orders id := by
    cases h.freshQueue rfl with
    | inl hz =>
      simp [hsucc] at hz
    | inr hf => exact hf
  have hlev : ({ price := px, orders := rid :: rest } : Level) ∈ levelsOf book side.opposite := by
    rw [hshapeS]
    exact mem_cons_self
  have hnot := not_passive h.pos h.cut (hfreshAll _ hlev rid mem_cons_self)
  have hlt : rid < incoming.id :=
    h.below (rid, resting) (findOrder_mem h.inv.keys hfind)
  refine {
    inv := bookInv_cancel h.inv rid
    ident := h.ident
    sideEq := h.sideEq
    limitEq := h.limitEq
    sumEq := h.sumEq
    spent := h.spent
    live := h.live
    pos := h.pos
    cut := ?_
    atLevel := h.atLevel
    worse := h.worse
    originQueue := origin_removeAt h.originQueue hlevels
    freshQueue := ?_
    below := ?_
    nextBound := ?_
  }
  · intro t ht
    have hne : t.passiveId ≠ o'.id := by simpa [o', hidr] using hnot t ht
    rcases h.cut t ht with ⟨before, after, hb, ha, hq⟩
    refine ⟨before, after, hb, ?_, hq⟩
    rw [horders, findOrder_upsert, ite_eq_right hne]
    exact ha
  · intro _
    exact Or.inr (fresh_after_edit h.inv hfreshAll hlev mem_cons_self rfl
      (by simp [o', hidr]) horders hlevels)
  · intro p hp
    have hp' : p ∈ upsert book.orders o' := by simpa [horders] using hp
    have ho' : o'.id < incoming.id := by simpa [o', hidr] using hlt
    exact below_upsert h.below ho' p hp'
  · rw [hc]
    simpa [nextId_dequeue, writeOrder] using h.nextBound

theorem matchAcc_fill
    (h : MatchAcc src budget side limit book incoming trades false)
    (policy : StpPolicy)
    {px : Int} {rid n : Nat} {rest : List Nat} {tail : List Level} {resting : Order}
    (hsucc : incoming.remaining = Nat.succ n)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (hact : resting.status.isActive = true)
    (hrest : 0 < resting.remaining)
    (hconf : stpConflict incoming resting policy = false) :
    MatchAcc src budget side limit
      (step book incoming policy false).book
      (step book incoming policy false).incoming
      (match (step book incoming policy false).trade with
        | none => trades
        | some t => t :: trades)
      (step book incoming policy false).stp := by
  let opp := incoming.side.opposite
  let qty := min incoming.remaining resting.remaining
  let emitted := emitTrade book px qty incoming.id rid incoming.side
  let incoming' := fill incoming qty
  let resting' := fill resting qty
  let book1 := writeOrder emitted.1 resting'
  let book2 := if resting'.remaining == 0 then dequeue book1 opp px rid else book1
  have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
  have hs : step book incoming policy false =
      { book := book2, incoming := incoming', stp := false, trade := some emitted.2, go := true } := by
    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty, emitted, incoming',
      resting', book1, book2, opp]
  have hk := step_progress h.inv incoming policy false
  have hinv' : BookInv book2 := by simpa [hs] using hk.inv
  have hidI : incoming'.id = incoming.id := by simp [incoming', fill]
  have hbelow' : ∀ p ∈ book2.orders, p.1 < incoming'.id := by
    simpa [hs, hidI] using hk.kept incoming.id h.below
  have hnext' : incoming'.id < book2.nextOrderId := by
    have hn : book2.nextOrderId = book.nextOrderId := by simpa [hs] using hk.nextId
    simpa [hidI, hn] using h.nextBound
  have hopp : opp = side.opposite := by simp [opp, h.sideEq]
  have hshapeS : levelsOf book side.opposite =
      { price := px, orders := rid :: rest } :: tail := by
    rw [← hopp]
    simpa [opp] using hshape
  have horders : book2.orders = upsert book.orders resting' := by
    have hemit : emitted.1.orders = book.orders := by simp [emitted, orders_emit]
    cases hdead : (resting'.remaining == 0)
    · simp [book2, hdead, book1, writeOrder, hemit]
    · simp [book2, hdead, book1, writeOrder, hemit, orders_dequeue]
  have hid' : resting'.id = rid := by
    have hidr := findOrder_id h.inv.keys hfind
    simp [resting', fill, hidr]
  have hfreshAll : ∀ level ∈ levelsOf book side.opposite, ∀ id ∈ level.orders,
      findOrder book.orders id = findOrder src.orders id := by
    cases h.freshQueue rfl with
    | inl hz =>
      simp [hsucc] at hz
    | inr hf => exact hf
  have hlev : ({ price := px, orders := rid :: rest } : Level) ∈ levelsOf book side.opposite := by
    rw [hshapeS]
    exact mem_cons_self
  have hnot := not_passive h.pos h.cut (hfreshAll _ hlev rid mem_cons_self)
  have hqpos : 0 < qty := Nat.lt_min.mpr ⟨by simp [hsucc], hrest⟩
  have hqtyLe : qty ≤ incoming.remaining := Nat.min_le_left _ _
  have hqtyRest : qty ≤ resting.remaining := Nat.min_le_right _ _
  have hfilled' : incoming'.filled = incoming.filled + qty := by simp [incoming', fill]
  have hrem' : incoming'.remaining = incoming.remaining - qty := by simp [incoming', fill]
  have hrest' : resting'.remaining = resting.remaining - qty := by simp [resting', fill]
  have htQty : emitted.2.quantity = qty := by simp [emitted, emitTrade]
  have htId : emitted.2.passiveId = rid := by simp [emitted, emitTrade]
  have htPx : emitted.2.price = px := by simp [emitted, emitTrade]
  have horigin : ∀ level ∈ levelsOf book2 side.opposite, ∀ id ∈ level.orders,
      ∃ level0, level0 ∈ levelsOf src side.opposite ∧
        level0.price = level.price ∧ id ∈ level0.orders := by
    cases hdead : (resting'.remaining == 0)
    · have hlevels : levelsOf book2 side.opposite = levelsOf book side.opposite := by
        simp only [book2, hdead]
        rw [ite_eq_right Bool.false_ne_true]
        simp only [book1, emitted]
        rw [levels_write, levels_emit]
      intro level hl id hid
      exact h.originQueue level (by simpa [hlevels] using hl) id hid
    · have hlevels : levelsOf book2 side.opposite =
          removeAt (levelsOf book side.opposite) px rid := by
        simp only [book2, hdead]
        rw [ite_eq_left trivial]
        simp only [book1, emitted, hopp]
        rw [levels_dequeue, levels_write, levels_emit]
      exact origin_removeAt h.originQueue hlevels
  have hfreshNew : incoming'.remaining = 0 ∨
      ∀ level ∈ levelsOf book2 side.opposite, ∀ id ∈ level.orders,
        findOrder book2.orders id = findOrder src.orders id := by
    cases hdead : (resting'.remaining == 0)
    · left
      have hneR : resting'.remaining ≠ 0 := by
        intro hz
        have hb : (resting'.remaining == 0) = true := by simp [hz]
        rw [hb] at hdead
        cases hdead
      have hnot0 : resting.remaining - qty ≠ 0 := by simpa [hrest'] using hneR
      cases Nat.lt_or_ge incoming.remaining resting.remaining with
      | inl hlt =>
        have hmin : qty = incoming.remaining := Nat.min_eq_left (Nat.le_of_lt hlt)
        simp [hrem', hmin]
      | inr hge =>
        have hmin : qty = resting.remaining := Nat.min_eq_right hge
        exact absurd (by simp [hmin]) hnot0
    · right
      have hlevels : levelsOf book2 side.opposite =
          removeAt (levelsOf book side.opposite) px rid := by
        simp only [book2, hdead]
        rw [ite_eq_left trivial]
        simp only [book1, emitted, hopp]
        rw [levels_dequeue, levels_write, levels_emit]
      exact fresh_after_edit h.inv hfreshAll hlev mem_cons_self rfl hid' horders hlevels
  have hacc : MatchAcc src budget side limit book2 incoming' (emitted.2 :: trades) false := {
    inv := hinv'
    ident := by simpa [hidI] using h.ident
    sideEq := by simpa [incoming', fill] using h.sideEq
    limitEq := by simpa [incoming', fill] using h.limitEq
    sumEq := by
      rw [tradeQtySum_cons, htQty, hfilled', h.sumEq]
    spent := by
      have hold := h.live rfl
      omega
    live := by
      intro _
      have hold := h.live rfl
      omega
    pos := by
      intro t ht
      rcases mem_cons.mp ht with rfl | ht
      · simpa [htQty] using hqpos
      · exact h.pos t ht
    cut := by
      intro t ht
      rcases mem_cons.mp ht with rfl | ht
      · refine ⟨resting, resting', ?_, ?_, ?_⟩
        · have hsrc : findOrder src.orders rid = some resting := by
            rw [← hfreshAll _ hlev rid mem_cons_self, hfind]
          simpa [htId] using hsrc
        · have hself := findOrder_upsert_self book.orders resting'
          simpa [horders, hid', htId] using hself
        · simp only [htQty, hrest']
          exact (Nat.sub_add_cancel hqtyRest).symm
      · have hne : t.passiveId ≠ resting'.id := by simpa [hid'] using hnot t ht
        rcases h.cut t ht with ⟨before, after, hb, ha, hq⟩
        refine ⟨before, after, hb, ?_, hq⟩
        rw [horders, findOrder_upsert, ite_eq_right hne]
        exact ha
    atLevel := by
      intro t ht
      rcases mem_cons.mp ht with rfl | ht
      · rcases h.originQueue _ hlev rid mem_cons_self with ⟨level0, hm, hp, hi⟩
        refine ⟨level0, hm, by simpa [htId] using hi, ?_⟩
        have hp' : level0.price = px := by simpa using hp
        simpa [htPx] using hp'.symm
      · exact h.atLevel t ht
    worse := by
      intro t ht
      rcases mem_cons.mp ht with rfl | ht
      · have hcross' : pricesCross side limit px = true := by
          simpa [h.sideEq, h.limitEq] using hcross
        cases side with
        | buy => simpa [withinLimit, htPx] using cross_buy hcross'
        | sell => simpa [withinLimit, htPx] using cross_sell hcross'
      · exact h.worse t ht
    originQueue := horigin
    freshQueue := fun _ => hfreshNew
    below := hbelow'
    nextBound := hnext'
  }
  simpa [hs] using hacc

theorem step_acc (policy : StpPolicy)
    (h : MatchAcc src budget side limit book incoming trades stp) :
    MatchAcc src budget side limit
      (step book incoming policy stp).book
      (step book incoming policy stp).incoming
      (match (step book incoming policy stp).trade with
        | none => trades
        | some t => t :: trades)
      (step book incoming policy stp).stp := by
  let opp := incoming.side.opposite
  cases stp
  case true =>
    have hs : step book incoming policy true =
        { book, incoming, stp := true, trade := none, go := false } := by
      simp [step]
    simpa [hs] using h
  case false =>
    cases hsucc : incoming.remaining
    case zero =>
      have hs : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc]
      simpa [hs] using h
    case succ n =>
      cases hbest : bestPrice (levelsOf book opp)
      case none =>
        have hs : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hsucc, hbest, opp]
        simpa [hs] using h
      case some px =>
        cases hcross : pricesCross incoming.side incoming.price px
        case false =>
          have hs : step book incoming policy false =
              { book, incoming, stp := false, trade := none, go := false } := by
            simp [step, hsucc, hbest, hcross, opp]
          simpa [hs] using h
        case true =>
          rcases front_order h.inv opp hbest with
            ⟨rid, resting, rest, tail, hshape, hfront, hfind, hsideR, hpxR, hact, hrest⟩
          have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
          cases policy
          case off =>
            exact matchAcc_fill h .off hsucc hbest hcross hshape hfront hfind
              hact hrest (stpConflict_off incoming resting)
          case cancelNewest =>
            cases hconf : stpConflict incoming resting .cancelNewest
            case false =>
              exact matchAcc_fill h .cancelNewest hsucc hbest hcross hshape hfront hfind
                hact hrest hconf
            case true =>
              have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
              have hs : step book incoming .cancelNewest false =
                  { book, incoming := stopIncoming incoming, stp := true, trade := none,
                    go := false } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq, opp]
              simpa [hs] using matchAcc_stop h
          case cancelOldest =>
            cases hconf : stpConflict incoming resting .cancelOldest
            case false =>
              exact matchAcc_fill h .cancelOldest hsucc hbest hcross hshape hfront hfind
                hact hrest hconf
            case true =>
              have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
              have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
              have hs : step book incoming .cancelOldest false =
                  { book := cancelOrder book rid, incoming, stp := false, trade := none,
                    go := true } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hsame,
                  opp]
              simpa [hs] using
                matchAcc_cancel h hsucc hshape hfind hsideR hpxR hact
          case decrementAndCancel =>
            cases hconf : stpConflict incoming resting .decrementAndCancel
            case false =>
              exact matchAcc_fill h .decrementAndCancel hsucc hbest hcross hshape hfront hfind
                hact hrest hconf
            case true =>
              have hnotNew : (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
              have hnotOld : (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
              have hdec : (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
              by_cases hltR : incoming.remaining < resting.remaining
              · have hltN : n + 1 < resting.remaining := by simpa [hsucc] using hltR
                have hs : step book incoming .decrementAndCancel false =
                    { book, incoming := stopIncoming incoming, stp := true, trade := none,
                      go := false } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hnotOld, hdec, hltN, opp]
                simpa [hs] using matchAcc_stop h
              · have hnotN : ¬ n + 1 < resting.remaining := by simpa [hsucc] using hltR
                have hs : step book incoming .decrementAndCancel false =
                    { book := cancelOrder book rid, incoming, stp := false, trade := none,
                      go := true } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hnotOld, hdec, hnotN, opp]
                simpa [hs] using
                  matchAcc_cancel h hsucc hshape hfind hsideR hpxR hact

theorem matchLoop_acc (fuel : Nat) (policy : StpPolicy)
    (h : MatchAcc src budget side limit book incoming trades stp) :
    MatchAcc src budget side limit
      (matchLoop fuel book incoming policy trades stp).1
      (matchLoop fuel book incoming policy trades stp).2.1
      (matchLoop fuel book incoming policy trades stp).2.2.1
      (matchLoop fuel book incoming policy trades stp).2.2.2 := by
  induction fuel generalizing book incoming trades stp with
  | zero =>
    simpa [matchLoop] using matchAcc_reverse h
  | succ n ih =>
    have hstep := step_acc policy h
    cases hgo : (step book incoming policy stp).go
    · simp only [matchLoop, hgo]
      exact matchAcc_reverse hstep
    · simp only [matchLoop, hgo]
      exact ih hstep

theorem alloc_acc (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) :
    MatchAcc book quantity side price
      (alloc book side price quantity tif owner).1
      (alloc book side price quantity tif owner).2 [] false := by
  have halloc := bookInv_alloc h side price quantity tif owner
  let out := alloc book side price quantity tif owner
  change MatchAcc book quantity side price out.1 out.2 [] false
  exact {
    inv := by simpa [out] using halloc.1
    ident := by simpa [out] using halloc.2.1
    sideEq := by simp [out, alloc]
    limitEq := by simp [out, alloc]
    sumEq := by simp [out, alloc, tradeQtySum]
    spent := by simp [out, alloc]
    live := by intro _; simp [out, alloc]
    pos := by intro t ht; simp at ht
    cut := by intro t ht; simp at ht
    atLevel := by intro t ht; simp at ht
    worse := by intro t ht; simp at ht
    originQueue := by
      intro level hl id hid
      refine ⟨level, ?_, rfl, hid⟩
      cases side <;> simpa [out, alloc, levelsOf] using hl
    freshQueue := fun _ => Or.inr <| by
      intro level hl id hid
      simp [out, alloc]
    below := by simpa [out] using halloc.2.2.1
    nextBound := by simpa [out] using halloc.2.2.2
  }

structure SubmitFacts (src : Book) (side : Side) (limit : Int) (budget : Nat)
    (book' : Book) (trades : List Trade) : Prop where
  sumFilled : tradeQtySum trades = filledQty book' src.nextOrderId
  filledLe : filledQty book' src.nextOrderId ≤ budget
  account : budget =
    filledQty book' src.nextOrderId + restedQty book' src.nextOrderId +
      cancelledQty book' src.nextOrderId budget
  pos : ∀ t ∈ trades, 0 < t.quantity
  cut : ∀ t ∈ trades, ∃ before after,
    findOrder src.orders t.passiveId = some before ∧
    findOrder book'.orders t.passiveId = some after ∧
    before.remaining = after.remaining + t.quantity
  atLevel : ∀ t ∈ trades, ∃ level, level ∈ levelsOf src side.opposite ∧
    t.passiveId ∈ level.orders ∧ t.price = level.price
  worse : ∀ t ∈ trades, withinLimit side limit t.price

theorem find_settle_of_ne {book : Book} {o : Order} {stp : Bool} {id : Nat}
    (hne : id ≠ o.id) :
    findOrder (settle book o stp).orders id = findOrder book.orders id := by
  cases stp
  case true =>
    simp [settle, writeOrder, findOrder_upsert, hne]
  case false =>
    cases hzero : (o.remaining == 0)
    case true =>
      simp [settle, hzero, writeOrder, findOrder_upsert, hne]
    case false =>
      cases htif : (o.timeInForce == .gtc)
      case true =>
        simp [settle, hzero, htif, orders_enqueue, writeOrder, findOrder_upsert, hne]
      case false =>
        simp [settle, hzero, htif, writeOrder, findOrder_upsert, hne]

theorem filled_rested_of {book : Book} {id : Nat} {o : Order}
    (h : findOrder book.orders id = some o) :
    filledQty book id = o.filled ∧
      restedQty book id = if o.status.isActive then o.remaining else 0 := by
  simp [filledQty, restedQty, h]

theorem facts_of_qty
    (h : MatchAcc src budget side limit book incoming trades stp)
    {r : Nat}
    (hfilled : filledQty (settle book incoming stp) incoming.id = incoming.filled)
    (hrested : restedQty (settle book incoming stp) incoming.id = r)
    (hle : incoming.filled + r ≤ budget) :
    SubmitFacts src side limit budget (settle book incoming stp) trades where
  sumFilled := by
    have hfin : filledQty (settle book incoming stp) src.nextOrderId = incoming.filled := by
      simpa [h.ident] using hfilled
    rw [hfin]
    exact h.sumEq
  filledLe := by
    have hfin : filledQty (settle book incoming stp) src.nextOrderId = incoming.filled := by
      simpa [h.ident] using hfilled
    omega
  account := by
    have hfin : filledQty (settle book incoming stp) src.nextOrderId = incoming.filled := by
      simpa [h.ident] using hfilled
    have hres : restedQty (settle book incoming stp) src.nextOrderId = r := by
      simpa [h.ident] using hrested
    simp only [cancelledQty, hfin, hres]
    exact nat_split budget incoming.filled r hle
  pos := h.pos
  cut := by
    intro t ht
    have hne : t.passiveId ≠ incoming.id := by
      rcases h.cut t ht with ⟨_, after, _, ha, _⟩
      exact Nat.ne_of_lt (h.below (t.passiveId, after) (findOrder_mem h.inv.keys ha))
    rcases h.cut t ht with ⟨before, after, hb, ha, hq⟩
    refine ⟨before, after, hb, ?_, hq⟩
    rw [find_settle_of_ne hne]
    exact ha
  atLevel := h.atLevel
  worse := h.worse

theorem write_filled (book : Book) (o stored : Order)
    (hid : stored.id = o.id) (hfill : stored.filled = o.filled) :
    filledQty (writeOrder book stored) o.id = o.filled := by
  simp [filledQty, writeOrder, findOrder_upsert, hid, hfill]

theorem write_rested (book : Book) (o stored : Order)
    (hid : stored.id = o.id) (hact : stored.status.isActive = false) :
    restedQty (writeOrder book stored) o.id = 0 := by
  simp [restedQty, writeOrder, findOrder_upsert, hid, hact]

/-- Filled quantity stays. Rested quantity is the live remainder, or zero. -/
theorem settle_qty (book : Book) (o : Order) (stp : Bool) :
    filledQty (settle book o stp) o.id = o.filled ∧
      restedQty (settle book o stp) o.id =
        if stp = true then 0
        else if (o.remaining == 0) = true then 0
        else if (o.timeInForce == .gtc) = true then o.remaining
        else 0 := by
  cases stp
  case true =>
    let stored : Order := { o with status := .cancelled }
    have hact : stored.status.isActive = false := by simp [stored, OrderStatus.isActive]
    refine ⟨?_, ?_⟩
    · simpa [settle, stored] using write_filled book o stored rfl rfl
    · simpa [settle, stored] using write_rested book o stored rfl hact
  case false =>
    cases hzero : (o.remaining == 0)
    case true =>
      let stored : Order := { o with status := .filled }
      have hact : stored.status.isActive = false := by simp [stored, OrderStatus.isActive]
      refine ⟨?_, ?_⟩
      · simpa [settle, hzero, stored] using write_filled book o stored rfl rfl
      · simpa [settle, hzero, stored] using write_rested book o stored rfl hact
    case false =>
      cases htif : (o.timeInForce == .gtc)
      case false =>
        let stored : Order := { o with remaining := 0, status := .cancelled }
        have hact : stored.status.isActive = false := by simp [stored, OrderStatus.isActive]
        refine ⟨?_, ?_⟩
        · simpa [settle, hzero, htif, stored] using write_filled book o stored rfl rfl
        · simpa [settle, hzero, htif, stored] using write_rested book o stored rfl hact
      case true =>
        refine ⟨?_, ?_⟩
        · simp [filledQty, settle, hzero, htif, orders_enqueue, writeOrder, findOrder_upsert]
        · simp [restedQty, settle, hzero, htif, orders_enqueue, writeOrder, findOrder_upsert,
            OrderStatus.isActive]
          split <;> simp

theorem settle_facts
    (h : MatchAcc src budget side limit book incoming trades stp) :
    SubmitFacts src side limit budget (settle book incoming stp) trades := by
  rcases settle_qty book incoming stp with ⟨hfilled, hrested⟩
  cases stp
  case true =>
    have hzeroR : restedQty (settle book incoming true) incoming.id = 0 := by
      simpa using hrested
    have hle : incoming.filled + 0 ≤ budget := by
      have := h.spent
      omega
    exact facts_of_qty h hfilled hzeroR hle
  case false =>
    cases hrem : (incoming.remaining == 0)
    case true =>
      have hzeroR : restedQty (settle book incoming false) incoming.id = 0 := by
        simpa [hrem] using hrested
      have hle : incoming.filled + 0 ≤ budget := by
        have := h.spent
        omega
      exact facts_of_qty h hfilled hzeroR hle
    case false =>
      cases htif : (incoming.timeInForce == .gtc)
      case true =>
        have hliveR : restedQty (settle book incoming false) incoming.id = incoming.remaining := by
          simpa [hrem, htif] using hrested
        exact facts_of_qty h hfilled hliveR h.spent
      case false =>
        have hzeroR : restedQty (settle book incoming false) incoming.id = 0 := by
          simpa [hrem, htif] using hrested
        have hle : incoming.filled + 0 ≤ budget := by
          have := h.spent
          omega
        exact facts_of_qty h hfilled hzeroR hle

theorem fok_facts (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) :
    SubmitFacts book side price quantity (alloc book side price quantity tif owner).1 [] := by
  have halloc := bookInv_alloc h side price quantity tif owner
  have horders : (alloc book side price quantity tif owner).1.orders = book.orders := by
    simp [alloc]
  have hid : (alloc book side price quantity tif owner).2.id = book.nextOrderId := by
    simp [alloc]
  have hnone : findOrder (alloc book side price quantity tif owner).1.orders book.nextOrderId =
      none := by
    apply findOrder_below
    intro p hp
    have hlt := halloc.2.2.1 p hp
    simpa [hid] using hlt
  exact {
    sumFilled := by simp [tradeQtySum, filledQty, hnone]
    filledLe := by simp [filledQty, hnone]
    account := by simp [cancelledQty, filledQty, restedQty, hnone]
    pos := by intro _ ht; cases ht
    cut := by intro _ ht; cases ht
    atLevel := by intro _ ht; cases ht
    worse := by intro _ ht; cases ht
  }

theorem submit_facts (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) (policy : StpPolicy) :
    SubmitFacts book side price quantity
      (submit book side price quantity tif owner policy).1
      (submit book side price quantity tif owner policy).2 := by
  unfold submit
  split
  · simpa using fok_facts h side price quantity tif owner
  · let out := alloc book side price quantity tif owner
    let looped := matchLoop (matchFuel out.1 out.2) out.1 out.2 policy [] false
    have hacc : MatchAcc book quantity side price out.1 out.2 [] false := by
      simpa [out] using alloc_acc h side price quantity tif owner
    have hloop := matchLoop_acc (matchFuel out.1 out.2) policy hacc
    simpa [out, looped] using settle_facts hloop

theorem apply_submit_facts (h : BookInv book) (event : Event) (view : SubmitView)
    (hview : eventSubmit event = some view) :
    SubmitFacts book view.side view.price view.quantity (apply book event).1 (apply book event).2 := by
  cases event with
  | cancel _ =>
    simp [eventSubmit] at hview
  | submitLimit side price quantity tif owner policy =>
    simp only [eventSubmit] at hview
    injection hview with hview
    cases hview
    simpa [apply] using submit_facts h side price quantity tif owner policy
  | submitMarket side quantity owner policy =>
    cases side with
    | buy =>
      simp only [eventSubmit] at hview
      injection hview with hview
      cases hview
      simpa [apply] using submit_facts h .buy i64Max quantity .ioc owner policy
    | sell =>
      simp only [eventSubmit] at hview
      injection hview with hview
      cases hview
      simpa [apply] using submit_facts h .sell i64Min quantity .ioc owner policy

theorem law3_holds : law3 := by
  intro book hReach event view book' trades hview happ
  have hf := apply_submit_facts (reachable_inv hReach) event view hview
  have hf' : SubmitFacts book view.side view.price view.quantity book' trades := by
    simpa [happ] using hf
  dsimp
  exact ⟨hf'.sumFilled, hf'.filledLe, hf'.account, hf'.pos, hf'.cut⟩
