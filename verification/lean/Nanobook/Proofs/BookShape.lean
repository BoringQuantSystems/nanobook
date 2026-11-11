/-
Queue shape of a reachable book.
An active order stands once, on its side, at its price.
-/

import Nanobook.Proofs.WellFormed

namespace Nanobook

open List

def keysOnce (orders : List (Nat × Order)) : Prop :=
  (orders.map Prod.fst).Nodup

/-- An active stored order has its id in a level. -/
def activeInQueue (book : Book) : Prop :=
  ∀ p ∈ book.orders, p.2.status.isActive = true → p.2.id ∈ queueIds book

structure Placed (book : Book) : Prop where
  inQueue : activeInQueue book
  uniqueKeys : keysOnce book.orders

theorem count_one {a : Nat} {l : List Nat} (h : l.Nodup) (hm : a ∈ l) : l.count a = 1 := by
  induction l with
  | nil => cases hm
  | cons x xs ih =>
    rcases nodup_cons.mp h with ⟨hx, hxs⟩
    rw [List.count_cons]
    rcases mem_cons.mp hm with rfl | hm
    · have hz : xs.count a = 0 := List.count_eq_zero_of_not_mem hx
      simp [hz]
    · have hne : x ≠ a := by
        intro heq
        exact hx (heq ▸ hm)
      simp [beq_eq_false_iff_ne.mpr hne, ih hxs hm]

theorem mem_map_upsert {orders : List (Nat × Order)} {o : Order} {k : Nat}
    (h : k ∈ (upsert orders o).map Prod.fst) : k = o.id ∨ k ∈ orders.map Prod.fst := by
  induction orders with
  | nil =>
    have hup : upsert ([] : List (Nat × Order)) o = [(o.id, o)] := by simp [upsert]
    rw [hup, map_singleton, mem_singleton] at h
    exact Or.inl h
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    by_cases hi : i = o.id
    · have hup : upsert ((i, prev) :: tail) o = (o.id, o) :: tail := by simp [upsert, hi]
      rw [hup, map_cons, mem_cons] at h
      rcases h with rfl | h
      · exact Or.inl rfl
      · exact Or.inr (mem_cons_of_mem _ h)
    · have hup : upsert ((i, prev) :: tail) o = (i, prev) :: upsert tail o := by simp [upsert, hi]
      rw [hup, map_cons, mem_cons] at h
      rcases h with rfl | h
      · exact Or.inr mem_cons_self
      · rcases ih h with hnew | hold
        · exact Or.inl hnew
        · exact Or.inr (mem_cons_of_mem _ hold)

theorem keysOnce_upsert {orders : List (Nat × Order)} (h : keysOnce orders) (o : Order) :
    keysOnce (upsert orders o) := by
  induction orders with
  | nil =>
    unfold keysOnce
    simp [upsert]
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    unfold keysOnce at h ⊢
    have hcons := nodup_cons.mp h
    by_cases hi : i = o.id
    · have hup : upsert ((i, prev) :: tail) o = (o.id, o) :: tail := by simp [upsert, hi]
      rw [hi] at h
      rw [hup, map_cons]
      exact h
    · have hup : upsert ((i, prev) :: tail) o = (i, prev) :: upsert tail o := by simp [upsert, hi]
      rw [hup, map_cons]
      refine nodup_cons.mpr ⟨?_, ih hcons.2⟩
      intro hm
      rcases mem_map_upsert hm with hnew | hold
      · exact hi (by simpa using hnew)
      · exact hcons.1 hold

theorem mem_upsert_unique {orders : List (Nat × Order)} {o : Order} {p : Nat × Order}
    (hk : keysOnce orders) (hp : p ∈ upsert orders o) :
    p = (o.id, o) ∨ p ∈ orders ∧ p.1 ≠ o.id := by
  induction orders with
  | nil =>
    have hup : upsert ([] : List (Nat × Order)) o = [(o.id, o)] := by simp [upsert]
    rw [hup, mem_singleton] at hp
    exact Or.inl hp
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    unfold keysOnce at hk
    have hcons := nodup_cons.mp hk
    have htail : keysOnce tail := hcons.2
    by_cases hi : i = o.id
    · have hup : upsert ((i, prev) :: tail) o = (o.id, o) :: tail := by simp [upsert, hi]
      rw [hup, mem_cons] at hp
      rcases hp with rfl | hp
      · exact Or.inl rfl
      · refine Or.inr ⟨mem_cons_of_mem _ hp, ?_⟩
        intro hp1
        apply hcons.1
        have hmem : p.1 ∈ tail.map Prod.fst := mem_map.mpr ⟨p, hp, rfl⟩
        have hfst : (i, prev).fst = p.1 := by simp [hi, hp1]
        exact hfst ▸ hmem
    · have hup : upsert ((i, prev) :: tail) o = (i, prev) :: upsert tail o := by simp [upsert, hi]
      rw [hup, mem_cons] at hp
      rcases hp with rfl | hp
      · exact Or.inr ⟨mem_cons_self, hi⟩
      · rcases ih htail hp with rfl | ⟨hm, hne⟩
        · exact Or.inl rfl
        · exact Or.inr ⟨mem_cons_of_mem _ hm, hne⟩

theorem findOrder_of_mem {orders : List (Nat × Order)}
    (hkeys : keysMatch orders) (hk : keysOnce orders) {i : Nat} {o : Order}
    (h : (i, o) ∈ orders) : findOrder orders i = some o := by
  induction orders with
  | nil => cases h
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    unfold keysOnce at hk
    have hcons := nodup_cons.mp hk
    have hkeysTail : keysMatch tail := fun p hp => hkeys p (mem_cons_of_mem _ hp)
    have hktail : keysOnce tail := hcons.2
    cases hb : (k == i)
    · simp [findOrder, hb]
      rcases mem_cons.mp h with hhead | htail
      · cases hhead
        simp at hb
      · exact ih hkeysTail hktail htail
    · simp [findOrder, hb]
      have hkEq : k = i := by simpa using hb
      subst hkEq
      rcases mem_cons.mp h with hhead | htail
      · cases hhead
        rfl
      · exact absurd (mem_map_of_mem (f := Prod.fst) htail) hcons.1

theorem entry_unique (hkeys : keysMatch orders) (hk : keysOnce orders) {id : Nat} {o : Order}
    (hfind : findOrder orders id = some o) {p : Nat × Order} (hp : p ∈ orders) (hp1 : p.1 = id) :
    p = (id, o) := by
  have hf := findOrder_of_mem hkeys hk (show (p.1, p.2) ∈ orders from hp)
  rw [hp1, hfind] at hf
  cases hf
  rcases p with ⟨_, _⟩
  simp at hp1
  subst hp1
  rfl

theorem mem_removeId_of_ne {ids : List Nat} {j id : Nat} (hne : j ≠ id) (hj : j ∈ ids) :
    j ∈ removeId ids id := by
  induction ids with
  | nil => cases hj
  | cons x xs ih =>
    cases hb : (x == id)
    · have hstep : removeId (x :: xs) id = x :: removeId xs id := by simp [removeId, hb]
      rw [hstep]
      rcases mem_cons.mp hj with rfl | hj
      · exact mem_cons_self
      · exact mem_cons_of_mem x (ih hj)
    · have hstep : removeId (x :: xs) id = xs := by simp [removeId, hb]
      rw [hstep]
      rcases mem_cons.mp hj with hhead | hj
      · have hx : x = id := by simpa using hb
        exact absurd (hhead.trans hx) hne
      · exact hj

theorem mem_flat_removeAt_of_ne {levels : List Level} {j id : Nat} {price : Int}
    (hne : j ≠ id) (hj : j ∈ levels.flatMap (·.orders)) :
    j ∈ (removeAt levels price id).flatMap (·.orders) := by
  induction levels with
  | nil => cases hj
  | cons lvl tail ih =>
    rw [flatMap_cons] at hj
    rcases mem_append.mp hj with hj | hj
    · cases hp : (lvl.price == price)
      · have hstep : removeAt (lvl :: tail) price id = lvl :: removeAt tail price id := by
          simp [removeAt, hp]
        rw [hstep, flatMap_cons]
        exact mem_append_left _ hj
      · cases hempty : (removeId lvl.orders id).isEmpty
        · have hstep : removeAt (lvl :: tail) price id =
              { lvl with orders := removeId lvl.orders id } :: tail := by
            simp [removeAt, hp, hempty]
          rw [hstep, flatMap_cons]
          exact mem_append_left _ (mem_removeId_of_ne hne hj)
        · have hm := mem_removeId_of_ne hne hj
          have hempty' : removeId lvl.orders id = [] := by simpa [List.isEmpty_iff] using hempty
          simp [hempty'] at hm
    · cases hp : (lvl.price == price)
      · have hstep : removeAt (lvl :: tail) price id = lvl :: removeAt tail price id := by
          simp [removeAt, hp]
        rw [hstep, flatMap_cons]
        exact mem_append_right _ (ih hj)
      · cases hempty : (removeId lvl.orders id).isEmpty
        · have hstep : removeAt (lvl :: tail) price id =
              { lvl with orders := removeId lvl.orders id } :: tail := by
            simp [removeAt, hp, hempty]
          rw [hstep, flatMap_cons]
          exact mem_append_right _ hj
        · have hstep : removeAt (lvl :: tail) price id = tail := by
            simp [removeAt, hp, hempty]
          rw [hstep]
          exact hj

theorem mem_queue_dequeue_ne (book : Book) (side : Side) (px : Int) (id j : Nat)
    (hne : j ≠ id) (hj : j ∈ queueIds book) :
    j ∈ queueIds (dequeue book side px id) := by
  rw [queueIds] at hj
  cases side with
  | buy =>
    have hq : queueIds (dequeue book .buy px id) =
        (removeAt book.bids px id).flatMap (·.orders) ++ book.asks.flatMap (·.orders) := by
      simp [queueIds, dequeue, setLevels, levelsOf]
    rw [hq]
    rcases mem_append.mp hj with hj | hj
    · exact mem_append_left _ (mem_flat_removeAt_of_ne (price := px) hne hj)
    · exact mem_append_right _ hj
  | sell =>
    have hq : queueIds (dequeue book .sell px id) =
        book.bids.flatMap (·.orders) ++ (removeAt book.asks px id).flatMap (·.orders) := by
      simp [queueIds, dequeue, setLevels, levelsOf]
    rw [hq]
    rcases mem_append.mp hj with hj | hj
    · exact mem_append_left _ hj
    · exact mem_append_right _ (mem_flat_removeAt_of_ne (price := px) hne hj)

theorem placed_move {a b : Book} (hp : Placed a) (ho : b.orders = a.orders)
    (hq : queueIds b = queueIds a) : Placed b := by
  refine ⟨?_, ?_⟩
  · intro p hm hact
    have hm' : p ∈ a.orders := by simpa [ho] using hm
    simpa [hq] using hp.inQueue p hm' hact
  · simpa [keysOnce, ho] using hp.uniqueKeys

theorem write_queue (book : Book) (o : Order) :
    queueIds (writeOrder book o) = queueIds book := by
  simp [queueIds, writeOrder]

theorem placed_write (hs : Placed book) (o' : Order)
    (hok : o'.status.isActive = true → o'.id ∈ queueIds book) :
    Placed (writeOrder book o') := by
  refine ⟨?_, keysOnce_upsert hs.uniqueKeys o'⟩
  intro p hp hact
  have hp' : p ∈ upsert book.orders o' := by simpa [writeOrder] using hp
  rcases mem_upsert_unique hs.uniqueKeys hp' with rfl | ⟨hold, _hne⟩
  · simpa [queueIds, writeOrder] using hok hact
  · simpa [queueIds, writeOrder] using hs.inQueue p hold hact

theorem placed_remove (hkeys : keysMatch book.orders) (hs : Placed book) {id : Nat} {o : Order}
    (hfind : findOrder book.orders id = some o) (hinact : o.status.isActive = false)
    (side : Side) (px : Int) : Placed (dequeue book side px id) := by
  have ho : (dequeue book side px id).orders = book.orders := by simp [dequeue, orders_setLevels]
  refine ⟨?_, ?_⟩
  · intro p hp hact
    have hp0 : p ∈ book.orders := by simpa [ho] using hp
    have hne : p.2.id ≠ id := by
      intro heq
      have hp1 : p.1 = id := (hkeys p hp0).trans heq
      have huniq := entry_unique hkeys hs.uniqueKeys hfind hp0 hp1
      have hactO : o.status.isActive = true := by simpa [huniq] using hact
      cases hactO.symm.trans hinact
    exact mem_queue_dequeue_ne book side px id p.2.id hne (hs.inQueue p hp0 hact)
  · simpa [keysOnce, ho] using hs.uniqueKeys

theorem placed_emit (hs : Placed book) (price : Int) (qty aggressor passive : Nat) (side : Side) :
    Placed (emitTrade book price qty aggressor passive side).1 := by
  refine placed_move hs ?_ ?_
  · simp [emitTrade]
  · simp [queueIds, emitTrade]

theorem level_head_queued {book : Book} {side : Side} {px : Int} {id : Nat} {rest : List Nat}
    {tail : List Level}
    (h : levelsOf book side = ({ price := px, orders := id :: rest } : Level) :: tail) :
    id ∈ queueIds book := by
  cases side with
  | buy =>
    have hb : book.bids = { price := px, orders := id :: rest } :: tail := by
      simpa [levelsOf] using h
    have hid : id ∈ book.bids.flatMap (·.orders) := by
      rw [hb, flatMap_cons]
      exact mem_append_left _ mem_cons_self
    rw [queueIds]
    exact mem_append_left _ hid
  | sell =>
    have ha : book.asks = { price := px, orders := id :: rest } :: tail := by
      simpa [levelsOf] using h
    have hid : id ∈ book.asks.flatMap (·.orders) := by
      rw [ha, flatMap_cons]
      exact mem_append_left _ mem_cons_self
    rw [queueIds]
    exact mem_append_right _ hid

theorem placed_fill (hinv : BookInv book) (hs : Placed book) (incoming resting : Order)
    (policy : StpPolicy) {px : Int} {rid n : Nat} {rest : List Nat} {tail : List Level}
    (hsucc : incoming.remaining = Nat.succ n)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hact : resting.status.isActive = true) (hrest : 0 < resting.remaining)
    (hconf : stpConflict incoming resting policy = false) :
    Placed (step book incoming policy false).book := by
  let opp := incoming.side.opposite
  let qty := min incoming.remaining resting.remaining
  let emitted := emitTrade book px qty incoming.id rid incoming.side
  let incoming' := fill incoming qty
  let resting' := fill resting qty
  let book1 := writeOrder emitted.1 resting'
  let book2 := if resting'.remaining == 0 then dequeue book1 opp px rid else book1
  have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
  have hsEq : step book incoming policy false =
      { book := book2, incoming := incoming', stp := false, trade := some emitted.2, go := true } := by
    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty, emitted, incoming',
      resting', book1, book2, opp]
  have hidr : resting.id = rid := findOrder_id hinv.keys hfind
  have hsE : Placed emitted.1 := placed_emit hs px qty incoming.id rid incoming.side
  have hmem : resting'.id ∈ queueIds emitted.1 := by
    have hq : rid ∈ queueIds book := level_head_queued hshape
    simpa [resting', fill, hidr, emitted, emitTrade, queueIds] using hq
  have hp2 : Placed book2 := by
    cases hdead : (resting'.remaining == 0)
    · have hsW := placed_write hsE resting' (fun _ => hmem)
      simpa [book2, hdead] using hsW
    · have hzeroR : resting'.remaining = 0 := by simpa using hdead
      have hinact : resting'.status.isActive = false := by
        have hsub : resting.remaining - qty = 0 := by simpa [resting', fill] using hzeroR
        simp [resting', fill, hsub, OrderStatus.isActive]
      have hsW := placed_write hsE resting' (fun h => by simp [hinact] at h)
      have hkeysW : keysMatch book1.orders := by
        simpa [book1, writeOrder, emitted, emitTrade] using
          keysMatch_upsert book.orders resting' hinv.keys
      have hfindW : findOrder book1.orders rid = some resting' := by
        have hid' : resting'.id = rid := by simp [resting', fill, hidr]
        simpa [book1, writeOrder, hid'] using findOrder_upsert_self emitted.1.orders resting'
      have hsD := placed_remove hkeysW hsW hfindW hinact opp px
      simpa [book2, hdead] using hsD
  simpa [hsEq] using hp2

theorem placed_cancel (hinv : BookInv book) (hs : Placed book) (id : Nat) :
    Placed (cancelOrder book id) := by
  cases hfind : findOrder book.orders id with
  | none =>
    simpa [cancelOrder, hfind] using hs
  | some o =>
    cases hact : o.status.isActive
    · simpa [cancelOrder, hfind, hact] using hs
    · let o' : Order := { o with remaining := 0, status := .cancelled }
      have hinact : o'.status.isActive = false := by simp [o', OrderStatus.isActive]
      have hid : o'.id = id := by
        have hid0 := findOrder_id hinv.keys hfind
        simpa [o'] using hid0
      have hsW := placed_write hs o' (fun h => by simp [hinact] at h)
      have hkeysW : keysMatch (writeOrder book o').orders := by
        simpa [writeOrder] using keysMatch_upsert book.orders o' hinv.keys
      have hfindW : findOrder (writeOrder book o').orders id = some o' := by
        simpa [writeOrder, hid] using findOrder_upsert_self book.orders o'
      have hsD := placed_remove hkeysW hsW hfindW hinact o.side o.price
      simpa [cancelOrder, hfind, hact, o'] using hsD

theorem placed_step (hinv : BookInv book) (hs : Placed book) (incoming : Order)
    (policy : StpPolicy) (stp : Bool) : Placed (step book incoming policy stp).book := by
  cases stp
  case true =>
    have hsEq : step book incoming policy true =
        { book, incoming, stp := true, trade := none, go := false } := by
      simp [step]
    simpa [hsEq] using hs
  case false =>
    cases hsucc : incoming.remaining
    case zero =>
      have hsEq : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc]
      simpa [hsEq] using hs
    case succ n =>
      let opp := incoming.side.opposite
      cases hbest : bestPrice (levelsOf book opp)
      case none =>
        have hsEq : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hsucc, hbest, opp]
        simpa [hsEq] using hs
      case some px =>
        cases hcross : pricesCross incoming.side incoming.price px
        case false =>
          have hsEq : step book incoming policy false =
              { book, incoming, stp := false, trade := none, go := false } := by
            simp [step, hsucc, hbest, hcross, opp]
          simpa [hsEq] using hs
        case true =>
          rcases front_order hinv opp hbest with
            ⟨rid, resting, rest, tail, hshape, hfront, hfind, _hside, _hpx, hact, hrest⟩
          have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
          cases policy
          case off =>
            exact placed_fill hinv hs incoming resting .off hsucc hbest hcross hfront hfind hshape
              hact hrest (stpConflict_off incoming resting)
          case cancelNewest =>
            cases hconf : stpConflict incoming resting .cancelNewest
            case false =>
              exact placed_fill hinv hs incoming resting .cancelNewest hsucc hbest hcross hfront
                hfind hshape hact hrest hconf
            case true =>
              have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
              have hsEq : step book incoming .cancelNewest false =
                  { book, incoming := stopIncoming incoming, stp := true, trade := none,
                    go := false } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq, opp]
              simpa [hsEq] using hs
          case cancelOldest =>
            cases hconf : stpConflict incoming resting .cancelOldest
            case false =>
              exact placed_fill hinv hs incoming resting .cancelOldest hsucc hbest hcross hfront
                hfind hshape hact hrest hconf
            case true =>
              have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
              have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
              have hsEq : step book incoming .cancelOldest false =
                  { book := cancelOrder book rid, incoming, stp := false, trade := none,
                    go := true } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hsame,
                  opp]
              simpa [hsEq] using placed_cancel hinv hs rid
          case decrementAndCancel =>
            cases hconf : stpConflict incoming resting .decrementAndCancel
            case false =>
              exact placed_fill hinv hs incoming resting .decrementAndCancel hsucc hbest hcross
                hfront hfind hshape hact hrest hconf
            case true =>
              have hnotNew : (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
              have hnotOld : (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
              have hdec : (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
              by_cases hltR : incoming.remaining < resting.remaining
              · have hltN : n + 1 < resting.remaining := by simpa [hsucc] using hltR
                have hsEq : step book incoming .decrementAndCancel false =
                    { book, incoming := stopIncoming incoming, stp := true, trade := none,
                      go := false } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hnotOld, hdec, hltN, opp]
                simpa [hsEq] using hs
              · have hnotN : ¬ n + 1 < resting.remaining := by simpa [hsucc] using hltR
                have hsEq : step book incoming .decrementAndCancel false =
                    { book := cancelOrder book rid, incoming, stp := false, trade := none,
                      go := true } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hnotOld, hdec, hnotN, opp]
                simpa [hsEq] using placed_cancel hinv hs rid

theorem placed_matchLoop (fuel : Nat) (hinv : BookInv book) (hs : Placed book) (incoming : Order)
    (policy : StpPolicy) (rev : List Trade) (stp : Bool) :
    Placed (matchLoop fuel book incoming policy rev stp).1 := by
  induction fuel generalizing book incoming rev stp with
  | zero => simpa [matchLoop] using hs
  | succ n ih =>
    have hS := placed_step hinv hs incoming policy stp
    have hI := (step_progress hinv incoming policy stp).inv
    cases hgo : (step book incoming policy stp).go
    · rw [matchLoop]
      simp only [hgo]
      exact hS
    · have hrec := ih hI hS (step book incoming policy stp).incoming
        (match (step book incoming policy stp).trade with
          | none => rev
          | some t => t :: rev)
        (step book incoming policy stp).stp
      rw [matchLoop]
      simp only [hgo]
      exact hrec

theorem insertBid_has (levels : List Level) (price : Int) (id : Nat) :
    id ∈ (insertBid levels price id).flatMap (·.orders) := by
  induction levels with
  | nil => simp [insertBid]
  | cons _ tail ih =>
    simp only [insertBid]
    split
    · simp
    · split
      · simp
      · simp [ih]

theorem insertAsk_has (levels : List Level) (price : Int) (id : Nat) :
    id ∈ (insertAsk levels price id).flatMap (·.orders) := by
  induction levels with
  | nil => simp [insertAsk]
  | cons _ tail ih =>
    simp only [insertAsk]
    split
    · simp
    · split
      · simp
      · simp [ih]

theorem insertBid_keeps {levels : List Level} {price : Int} {id j : Nat}
    (hj : j ∈ levels.flatMap (·.orders)) :
    j ∈ (insertBid levels price id).flatMap (·.orders) := by
  induction levels with
  | nil => cases hj
  | cons lvl tail ih =>
    rw [flatMap_cons] at hj
    rcases mem_append.mp hj with hj | hj
    · simp only [insertBid]
      split
      · rw [flatMap_cons, flatMap_cons]
        exact mem_append_right _ (mem_append_left _ hj)
      · split
        · rw [flatMap_cons]
          exact mem_append_left _ (mem_append_left _ hj)
        · rw [flatMap_cons]
          exact mem_append_left _ hj
    · simp only [insertBid]
      split
      · rw [flatMap_cons, flatMap_cons]
        exact mem_append_right _ (mem_append_right _ hj)
      · split
        · rw [flatMap_cons]
          exact mem_append_right _ hj
        · rw [flatMap_cons]
          exact mem_append_right _ (ih hj)

theorem insertAsk_keeps {levels : List Level} {price : Int} {id j : Nat}
    (hj : j ∈ levels.flatMap (·.orders)) :
    j ∈ (insertAsk levels price id).flatMap (·.orders) := by
  induction levels with
  | nil => cases hj
  | cons lvl tail ih =>
    rw [flatMap_cons] at hj
    rcases mem_append.mp hj with hj | hj
    · simp only [insertAsk]
      split
      · rw [flatMap_cons, flatMap_cons]
        exact mem_append_right _ (mem_append_left _ hj)
      · split
        · rw [flatMap_cons]
          exact mem_append_left _ (mem_append_left _ hj)
        · rw [flatMap_cons]
          exact mem_append_left _ hj
    · simp only [insertAsk]
      split
      · rw [flatMap_cons, flatMap_cons]
        exact mem_append_right _ (mem_append_right _ hj)
      · split
        · rw [flatMap_cons]
          exact mem_append_right _ hj
        · rw [flatMap_cons]
          exact mem_append_right _ (ih hj)

theorem enqueue_orders (book : Book) (o : Order) : (enqueue book o).orders = book.orders := by
  cases h : o.side <;> simp [enqueue, h]

theorem enqueue_has_id (book : Book) (o : Order) : o.id ∈ queueIds (enqueue book o) := by
  cases hside : o.side with
  | buy =>
    have hq : queueIds (enqueue book o) =
        (insertBid book.bids o.price o.id).flatMap (·.orders) ++
          book.asks.flatMap (·.orders) := by
      simp [queueIds, enqueue, hside]
    rw [hq]
    exact mem_append_left _ (insertBid_has book.bids o.price o.id)
  | sell =>
    have hq : queueIds (enqueue book o) =
        book.bids.flatMap (·.orders) ++
          (insertAsk book.asks o.price o.id).flatMap (·.orders) := by
      simp [queueIds, enqueue, hside]
    rw [hq]
    exact mem_append_right _ (insertAsk_has book.asks o.price o.id)

theorem enqueue_keeps {book : Book} {o : Order} {j : Nat} (hj : j ∈ queueIds book) :
    j ∈ queueIds (enqueue book o) := by
  rw [queueIds] at hj
  cases hside : o.side with
  | buy =>
    have hq : queueIds (enqueue book o) =
        (insertBid book.bids o.price o.id).flatMap (·.orders) ++
          book.asks.flatMap (·.orders) := by
      simp [queueIds, enqueue, hside]
    rw [hq]
    rcases mem_append.mp hj with hj | hj
    · exact mem_append_left _ (insertBid_keeps hj)
    · exact mem_append_right _ hj
  | sell =>
    have hq : queueIds (enqueue book o) =
        book.bids.flatMap (·.orders) ++
          (insertAsk book.asks o.price o.id).flatMap (·.orders) := by
      simp [queueIds, enqueue, hside]
    rw [hq]
    rcases mem_append.mp hj with hj | hj
    · exact mem_append_left _ hj
    · exact mem_append_right _ (insertAsk_keeps hj)

/-- Write an order and put the same id on its side. -/
theorem placed_rest (hs : Placed book) (o o' : Order) (hid : o'.id = o.id) :
    Placed (enqueue (writeOrder book o') o) := by
  refine ⟨?_, ?_⟩
  · intro p hp hactp
    have hpU : p ∈ upsert book.orders o' := by
      simpa [writeOrder, enqueue_orders] using hp
    rcases mem_upsert_unique hs.uniqueKeys hpU with rfl | ⟨hold, _hne⟩
    · simpa [hid] using enqueue_has_id (writeOrder book o') o
    · exact enqueue_keeps (by simpa [write_queue] using hs.inQueue p hold hactp)
  · simpa [keysOnce, writeOrder, enqueue_orders] using keysOnce_upsert hs.uniqueKeys o'

theorem placed_settle (hs : Placed book) (o : Order) (stp : Bool) : Placed (settle book o stp) := by
  cases hstp : stp
  · cases hzero : (o.remaining == 0)
    · cases htif : (o.timeInForce == .gtc)
      · let o' : Order := { o with remaining := 0, status := .cancelled }
        have hinact : o'.status.isActive = false := by simp [o', OrderStatus.isActive]
        simpa [settle, hstp, hzero, htif, o'] using placed_write hs o' (fun h => by
          simp [hinact] at h)
      · let status := if o.filled > 0 then OrderStatus.partiallyFilled else OrderStatus.new
        let o' : Order := { o with status := status }
        simpa [settle, hstp, hzero, htif, status, o'] using placed_rest hs o o' rfl
    · let o' : Order := { o with status := .filled }
      have hinact : o'.status.isActive = false := by simp [o', OrderStatus.isActive]
      simpa [settle, hstp, hzero, o'] using placed_write hs o' (fun h => by
        simp [hinact] at h)
  · let o' : Order := { o with status := .cancelled }
    have hinact : o'.status.isActive = false := by simp [o', OrderStatus.isActive]
    simpa [settle, hstp, o'] using placed_write hs o' (fun h => by
      simp [hinact] at h)

theorem placed_alloc (hs : Placed book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) : Placed (alloc book side price quantity tif owner).1 := by
  refine placed_move hs ?_ ?_
  · simp [alloc]
  · simp [queueIds, alloc]

theorem placed_submit (hinv : BookInv book) (hs : Placed book) (side : Side) (price : Int)
    (quantity : Nat) (tif : TimeInForce) (owner : Option Nat) (policy : StpPolicy) :
    Placed (submit book side price quantity tif owner policy).1 := by
  unfold submit
  split
  · simpa using placed_alloc hs side price quantity tif owner
  · let out := alloc book side price quantity tif owner
    have halloc := bookInv_alloc hinv side price quantity tif owner
    have hpA : Placed out.1 := placed_alloc hs side price quantity tif owner
    let looped := matchLoop (matchFuel out.1 out.2) out.1 out.2 policy [] false
    have hpL : Placed looped.1 :=
      placed_matchLoop (matchFuel out.1 out.2) halloc.1 hpA out.2 policy [] false
    have hloop := matchLoop_keep (book := out.1) (matchFuel out.1 out.2) halloc.1 out.2 policy []
      false halloc.2.2.1 halloc.2.2.2
    simpa [out, looped] using placed_settle hpL looped.2.1 looped.2.2.2

theorem placed_apply (hinv : BookInv book) (hs : Placed book) (event : Event) :
    Placed (apply book event).1 := by
  cases event with
  | submitLimit side price quantity tif owner policy =>
    simpa [apply] using placed_submit hinv hs side price quantity tif owner policy
  | submitMarket side quantity owner policy =>
    cases side with
    | buy => simpa [apply] using placed_submit hinv hs .buy i64Max quantity .ioc owner policy
    | sell => simpa [apply] using placed_submit hinv hs .sell i64Min quantity .ioc owner policy
  | cancel id =>
    simpa [apply] using placed_cancel hinv hs id

theorem placed_foldl (hinv : BookInv book) (hs : Placed book) (events : List Event) :
    Placed (events.foldl (fun b ev => (apply b ev).1) book) := by
  induction events generalizing book with
  | nil => simpa using hs
  | cons event rest ih => exact ih (bookInv_apply hinv event) (placed_apply hinv hs event)

theorem placed_empty : Placed Book.empty where
  inQueue := by
    intro p hp
    simp [Book.empty] at hp
  uniqueKeys := by simp [keysOnce, Book.empty]

theorem placed_reach (events : List Event) : Placed (reach events) := by
  simpa [reach] using placed_foldl bookInv_empty placed_empty events

theorem active_once (h : BookInv book) (hp : Placed book) : activeQueuedOnce book := by
  intro p hmem hact
  have hidq : p.2.id ∈ queueIds book := hp.inQueue p hmem hact
  have hcount : (queueIds book).count p.2.id = 1 := count_one h.nodup hidq
  have hfind : findOrder book.orders p.2.id = some p.2 := by
    have hpm : (p.2.id, p.2) ∈ book.orders := by
      rcases p with ⟨k, ord⟩
      have hk := h.keys (k, ord) hmem
      simp only at hk
      subst hk
      exact hmem
    exact findOrder_of_mem h.keys hp.uniqueKeys hpm
  refine ⟨hcount, ?_⟩
  rcases mem_append.mp hidq with hb | ha
  · rcases mem_flatMap.mp hb with ⟨level, hl, hi⟩
    rcases h.agreeBid level hl p.2.id hi with ⟨o, hf, hside, hpx, _, _⟩
    have ho : o = p.2 := by
      rw [hfind] at hf
      cases hf
      rfl
    refine ⟨level, ?_, ?_, hi⟩
    · rw [ho] at hside
      rw [hside]
      exact hl
    · rw [ho] at hpx
      exact hpx.symm
  · rcases mem_flatMap.mp ha with ⟨level, hl, hi⟩
    rcases h.agreeAsk level hl p.2.id hi with ⟨o, hf, hside, hpx, _, _⟩
    have ho : o = p.2 := by
      rw [hfind] at hf
      cases hf
      rfl
    refine ⟨level, ?_, ?_, hi⟩
    · rw [ho] at hside
      rw [hside]
      exact hl
    · rw [ho] at hpx
      exact hpx.symm

theorem law1_holds : law1 := by
  intro book hreach
  rcases hreach with ⟨events, rfl⟩
  have hinv := reach_inv events
  have hp := placed_reach events
  exact ⟨hinv.bidOrder, hinv.askOrder, hinv.bidFull, hinv.askFull, hinv.nodup, hinv.agreeBid,
    hinv.agreeAsk, active_once hinv hp⟩

end Nanobook
