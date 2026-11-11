/-
Price-time order of one submit.
-/

import Nanobook.Proofs.Conservation

namespace Nanobook

open List

def queueOf (book : Book) (side : Side) : List Nat :=
  (levelsOf book side).flatMap (·.orders)

def consTrade (rev : List Trade) (trade : Option Trade) : List Trade :=
  match trade with
  | none => rev
  | some t => t :: rev

abbrev Priority (src book : Book) (opp : Side) (trades : List Trade) : Prop :=
  trades.map (·.passiveId) <+ queueOf src opp ∧
  ∀ id, skippedBeforeLast (queueOf src opp) (trades.map (·.passiveId)) id →
    ∃ before after,
      findOrder src.orders id = some before ∧
      before.status.isActive = true ∧
      findOrder book.orders id = some after ∧
      after.status = .cancelled ∧
      after.remaining = 0

/-- Queue seen while one submit matches. `pref` holds ids already left behind. -/
structure WalkCarry (src book : Book) (opp : Side) (rev : List Trade) : Type where
  pref : List Nat
  inv : BookInv book
  srcInv : BookInv src
  queue : queueOf src opp = pref ++ queueOf book opp
  openId : Option Nat
  clearSub : openId = none →
    (rev.map (·.passiveId)).reverse <+ pref ∧
    ∀ id, (queueOf book opp).head? = some id → id ∉ rev.map (·.passiveId)
  openSub : ∀ i, openId = some i →
    (queueOf book opp).head? = some i ∧
    ∃ earlier, (rev.map (·.passiveId)).reverse = earlier ++ [i] ∧ earlier <+ pref
  spent : ∀ id ∈ pref,
    id ∈ rev.map (·.passiveId) ∨
      ∃ o, findOrder book.orders id = some o ∧ o.status = .cancelled ∧ o.remaining = 0
  live : ∀ id ∈ pref, ∃ o, findOrder src.orders id = some o ∧ o.status.isActive = true
  fresh : ∀ id ∈ queueOf book opp,
    findOrder book.orders id = findOrder src.orders id ∨ openId = some id

abbrev StepOut (src book : Book) (opp : Side) (rev : List Trade)
    (incoming : Order) (policy : StpPolicy) (stp : Bool) : Prop :=
  ∃ w : WalkCarry src (step book incoming policy stp).book opp
      (consTrade rev (step book incoming policy stp).trade),
    (step book incoming policy stp).incoming.side.opposite = opp ∧
    ((step book incoming policy stp).go = true →
      w.openId = none ∨ (step book incoming policy stp).incoming.remaining = 0)

theorem appearsBefore_ne {q : List Nat} {id last : Nat}
    (h : appearsBefore q id last) (hd : q.Nodup) : id ≠ last := by
  induction q generalizing id last with
  | nil => simp [appearsBefore] at h
  | cons head tail ih =>
    simp [appearsBefore] at h
    rcases h with ⟨rfl, hlast⟩ | ⟨_, _, hrec⟩
    · intro heq
      exact (nodup_cons.mp hd).1 (heq ▸ hlast)
    · exact ih hrec (nodup_cons.mp hd).2

theorem mem_appears {q : List Nat} {id last : Nat}
    (h : appearsBefore q id last) : id ∈ q := by
  induction q generalizing id last with
  | nil => simp [appearsBefore] at h
  | cons head tail ih =>
    simp [appearsBefore] at h
    rcases h with ⟨rfl, _⟩ | ⟨_, _, hrec⟩
    · exact mem_cons_self
    · exact mem_cons_of_mem _ (ih hrec)

theorem appears_in_pref {pref rest : List Nat} {id last : Nat}
    (hd : (pref ++ rest).Nodup)
    (h : appearsBefore (pref ++ rest) id last)
    (hpos : last ∈ pref ∨ rest.head? = some last) : id ∈ pref := by
  induction pref generalizing id last rest with
  | nil =>
    cases hpos with
    | inl hmem => simp at hmem
    | inr hhead =>
      rcases head?_eq_some_iff.mp hhead with ⟨tail, rfl⟩
      simp only [nil_append] at h hd
      have h' : (last = id ∧ last ∈ tail) ∨
          (last ≠ id ∧ last ≠ last ∧ appearsBefore tail id last) := by
        simpa [appearsBefore] using h
      rcases h' with ⟨_, hlast⟩ | ⟨_, hne, _⟩
      · exact absurd hlast (nodup_cons.mp hd).1
      · exact absurd rfl hne
  | cons a pref ih =>
    have hdCons : (a :: (pref ++ rest)).Nodup := by simpa [cons_append] using hd
    have hd' : (pref ++ rest).Nodup := (nodup_cons.mp hdCons).2
    simp [appearsBefore, cons_append] at h
    rcases h with ⟨rfl, _⟩ | ⟨_, hneLast, hrec⟩
    · exact mem_cons_self
    · have hpos' : last ∈ pref ∨ rest.head? = some last := by
        cases hpos with
        | inl hmem =>
          simp [mem_cons] at hmem
          rcases hmem with rfl | hmem
          · exact absurd rfl hneLast
          · exact Or.inl hmem
        | inr hhead => exact Or.inr hhead
      exact mem_cons_of_mem _ (ih hd' hrec hpos')

theorem pref_disj {pref rest : List Nat} {a : Nat}
    (hd : (pref ++ rest).Nodup) (hp : a ∈ pref) (hr : a ∈ rest) : False := by
  exact absurd rfl ((nodup_append.mp hd).2.2 a hp a hr)

theorem suffix_out {pref suffix : List Nat} {rid id : Nat}
    (hd : (pref ++ rid :: suffix).Nodup) (hid : id ∈ suffix) : id ≠ rid := by
  intro heq
  exact (nodup_cons.mp (nodup_append.mp hd).2.1).1 (heq ▸ hid)

theorem flat_drop_head (px : Int) (rid : Nat) (rest : List Nat) (tail : List Level) :
    (removeAt ({ price := px, orders := rid :: rest } :: tail) px rid).flatMap (·.orders)
      = rest ++ tail.flatMap (·.orders) := by
  have hp : ((({ price := px, orders := rid :: rest } : Level).price) == px) = true := by simp
  have hid : (rid == rid) = true := by simp
  simp only [removeAt, removeId, hp, hid]
  by_cases hempty : rest.isEmpty
  · have hr : rest = [] := by simpa [isEmpty_iff] using hempty
    simp [hr]
  · simp [hempty]

theorem queue_front {book : Book} {side : Side} {px : Int} {rid : Nat}
    {rest : List Nat} {tail : List Level}
    (h : levelsOf book side = { price := px, orders := rid :: rest } :: tail) :
    queueOf book side = rid :: (rest ++ tail.flatMap (·.orders)) := by
  simp [queueOf, h]

theorem cancel_front (h : BookInv book) {side : Side} {px : Int} {rid : Nat}
    {resting : Order} {rest : List Nat} {tail : List Level}
    (hshape : levelsOf book side = { price := px, orders := rid :: rest } :: tail)
    (hfind : findOrder book.orders rid = some resting)
    (hside : resting.side = side) (hpx : resting.price = px)
    (hact : resting.status.isActive = true) :
    let o' : Order := { resting with remaining := 0, status := .cancelled }
    let book' := cancelOrder book rid
    BookInv book' ∧
      queueOf book' side = rest ++ tail.flatMap (·.orders) ∧
      findOrder book'.orders rid = some o' ∧
      ∀ id, id ≠ rid → findOrder book'.orders id = findOrder book.orders id := by
  let o' : Order := { resting with remaining := 0, status := .cancelled }
  have hidr : resting.id = rid := findOrder_id h.keys hfind
  have hid' : o'.id = rid := by simp [o', hidr]
  have hbook : cancelOrder book rid = dequeue (writeOrder book o') side px rid := by
    simpa [o', hside, hpx] using cancelOrder_active hfind hact
  have hinv : BookInv (cancelOrder book rid) := by
    simpa [hbook] using bookInv_replace_out h hfind hid' hside hpx
  have hqueue : queueOf (cancelOrder book rid) side =
      rest ++ tail.flatMap (·.orders) := by
    have hlev : levelsOf (cancelOrder book rid) side =
        removeAt (levelsOf book side) px rid := by
      rw [hbook]
      simp [levels_dequeue, levels_write]
    rw [queueOf, hlev, hshape]
    exact flat_drop_head px rid rest tail
  have horders : (cancelOrder book rid).orders = upsert book.orders o' := by
    rw [hbook]
    simp [orders_dequeue, writeOrder]
  have hself : findOrder (cancelOrder book rid).orders rid = some o' := by
    rw [horders, ← hid']
    exact findOrder_upsert_self book.orders o'
  have hkeep : ∀ id, id ≠ rid →
      findOrder (cancelOrder book rid).orders id = findOrder book.orders id := by
    intro id hne
    rw [horders, findOrder_upsert]
    simp [hid', hne]
  exact ⟨hinv, hqueue, hself, hkeep⟩

theorem queued_below (h : BookInv book) (side : Side) {id : Nat}
    (hid : id ∈ queueOf book side) : id < book.nextOrderId := by
  rcases mem_flatMap.mp hid with ⟨level, hl, hi⟩
  rcases side_agree h side level hl id hi with ⟨o, hf, _, _, _, _⟩
  exact h.below (id, o) (findOrder_mem h.keys hf)

theorem walk_says {src book : Book} {opp : Side} {rev : List Trade}
    (w : WalkCarry src book opp rev) : Priority src book opp rev.reverse := by
  simp only [Priority, map_reverse]
  have hd0 : (queueOf src opp).Nodup := side_flat_nodup w.srcInv opp
  have hd : (w.pref ++ queueOf book opp).Nodup := by simpa [w.queue] using hd0
  cases hid : w.openId with
  | none =>
    rcases w.clearSub hid with ⟨hsub, _⟩
    refine ⟨?_, ?_⟩
    · rw [w.queue]
      exact sublist_append_of_sublist_left (l₂ := queueOf book opp) hsub
    intro id hskip
    rcases hskip with ⟨last, hlast, hbefore, hnin⟩
    have hpref : last ∈ w.pref := hsub.subset (mem_of_getLast? hlast)
    have hin : id ∈ w.pref :=
      appears_in_pref hd (by simpa [w.queue] using hbefore) (Or.inl hpref)
    have hninR : id ∉ rev.map (·.passiveId) := by
      intro hmem
      exact hnin (by simpa [mem_reverse] using hmem)
    rcases w.spent id hin with htr | ⟨after, hf, hst, hrem⟩
    · exact absurd htr hninR
    · rcases w.live id hin with ⟨before, hb, hact⟩
      exact ⟨before, after, hb, hact, hf, hst, hrem⟩
  | some i =>
    rcases w.openSub i hid with ⟨hhead, earlier, hrev, hearl⟩
    rcases head?_eq_some_iff.mp hhead with ⟨tail, hq⟩
    refine ⟨?_, ?_⟩
    · rw [hrev]
      have hsub : earlier ++ [i] <+ w.pref ++ (i :: tail) :=
        hearl.append ((nil_sublist tail).cons_cons i)
      simpa [w.queue, hq] using hsub
    · intro id hskip
      rcases hskip with ⟨last, hlast, hbefore, hnin⟩
      have hlastI : last = i := by
        rw [hrev] at hlast
        simp [getLast?_append, getLast?_singleton] at hlast
        exact hlast.symm
      have hbefore' : appearsBefore (w.pref ++ i :: tail) id i := by
        simpa [w.queue, hq, hlastI] using hbefore
      have hd' : (w.pref ++ i :: tail).Nodup := by simpa [w.queue, hq] using hd
      have hin : id ∈ w.pref := appears_in_pref hd' hbefore' (Or.inr rfl)
      have hninR : id ∉ rev.map (·.passiveId) := by
        intro hmem
        exact hnin (by simpa [mem_reverse] using hmem)
      rcases w.spent id hin with htr | ⟨after, hf, hst, hrem⟩
      · exact absurd htr hninR
      · rcases w.live id hin with ⟨before, hb, hact⟩
        exact ⟨before, after, hb, hact, hf, hst, hrem⟩

theorem walk_static {src book : Book} {opp : Side} {rev : List Trade}
    {incoming incoming' : Order} {policy : StpPolicy} {stp stp' : Bool}
    (w : WalkCarry src book opp rev)
    (hs : step book incoming policy stp =
      { book, incoming := incoming', stp := stp', trade := none, go := false })
    (hside : incoming'.side = incoming.side)
    (hopp : incoming.side.opposite = opp) :
    StepOut src book opp rev incoming policy stp := by
  unfold StepOut
  rw [hs]
  exact ⟨w, by simp [hside, hopp], by intro hgo; cases hgo⟩

theorem walk_cancel {src book : Book} {opp : Side} {rev : List Trade}
    {px : Int} {rid : Nat} {resting : Order} {rest : List Nat} {tail : List Level}
    (w : WalkCarry src book opp rev)
    (hnone : w.openId = none)
    (hshape : levelsOf book opp = { price := px, orders := rid :: rest } :: tail)
    (hfind : findOrder book.orders rid = some resting)
    (hside : resting.side = opp) (hpx : resting.price = px)
    (hact : resting.status.isActive = true) :
    ∃ w' : WalkCarry src (cancelOrder book rid) opp rev, w'.openId = none := by
  let suffix := rest ++ tail.flatMap (·.orders)
  have hq := queue_front hshape
  have hd0 : (queueOf src opp).Nodup := side_flat_nodup w.srcInv opp
  have hd : (w.pref ++ rid :: suffix).Nodup := by
    simpa [w.queue, hq, suffix] using hd0
  have hpack := cancel_front w.inv hshape hfind hside hpx hact
  have hnot : rid ∉ w.pref := by
    intro hp
    exact pref_disj hd hp mem_cons_self
  refine ⟨{
    pref := w.pref ++ [rid]
    inv := hpack.1
    srcInv := w.srcInv
    queue := by
      have hsrc : queueOf src opp = w.pref ++ rid :: suffix := by simpa [hq, suffix] using w.queue
      have hnew : queueOf (cancelOrder book rid) opp = suffix := by simpa [suffix] using hpack.2.1
      simpa [hnew, suffix, append_assoc] using hsrc
    openId := none
    clearSub := by
      intro _
      refine ⟨sublist_append_of_sublist_left (w.clearSub hnone).1, ?_⟩
      intro id hhead
      have hid : id ∈ suffix := by
        have hnew : queueOf (cancelOrder book rid) opp = suffix := by
          simpa [suffix] using hpack.2.1
        rw [hnew] at hhead
        exact mem_of_head? hhead
      intro hin
      have hinP : id ∈ w.pref :=
        (w.clearSub hnone).1.subset (by simpa [mem_reverse] using hin)
      exact pref_disj hd hinP (mem_cons_of_mem rid hid)
    openSub := by
      intro _ hi
      cases hi
    spent := by
      intro id hid
      rw [mem_append] at hid
      rcases hid with hid | hid
      · have hne : id ≠ rid := by
          intro heq
          exact hnot (heq ▸ hid)
        rcases w.spent id hid with hin | ⟨o, hf, hs, hr⟩
        · exact Or.inl hin
        · exact Or.inr ⟨o, by rw [hpack.2.2.2 id hne, hf], hs, hr⟩
      · have heq : id = rid := mem_singleton.mp hid
        rw [heq]
        exact Or.inr ⟨{ resting with remaining := 0, status := .cancelled },
          by simpa [heq] using hpack.2.2.1, rfl, rfl⟩
    live := by
      intro id hid
      rw [mem_append] at hid
      rcases hid with hid | hid
      · exact w.live id hid
      · have heq : id = rid := mem_singleton.mp hid
        rw [heq]
        rcases w.fresh rid (by simp [hq]) with heq | hbad
        · rw [heq] at hfind
          exact ⟨resting, hfind, hact⟩
        · rw [hnone] at hbad
          cases hbad
    fresh := by
      intro id hid
      have hnew : queueOf (cancelOrder book rid) opp = suffix := by simpa [suffix] using hpack.2.1
      have hidS : id ∈ suffix := by simpa [hnew] using hid
      have hne : id ≠ rid := suffix_out hd hidS
      have hidOld : id ∈ queueOf book opp := by
        simpa [hq, suffix] using mem_cons_of_mem rid hidS
      rw [hpack.2.2.2 id hne]
      rcases w.fresh id hidOld with heq | hbad
      · exact Or.inl heq
      · rw [hnone] at hbad
        cases hbad
  }, rfl⟩

theorem incoming_filled (incoming resting : Order)
    (hdead : ((fill resting (min incoming.remaining resting.remaining)).remaining == 0) = false) :
    (fill incoming (min incoming.remaining resting.remaining)).remaining = 0 := by
  let qty := min incoming.remaining resting.remaining
  cases Nat.le_total incoming.remaining resting.remaining with
  | inl hle =>
    have hq : qty = incoming.remaining := by simpa [qty] using Nat.min_eq_left hle
    simp [fill, hq, qty]
  | inr hge =>
    have hq : qty = resting.remaining := by simpa [qty] using Nat.min_eq_right hge
    have hz : (fill resting qty).remaining = 0 := by simp [fill, hq]
    have : ((fill resting qty).remaining == 0) = true := by simp [hz]
    simp [qty] at this hdead
    rw [this] at hdead
    exact False.elim (hdead rfl)

theorem walk_done {src book book' : Book} {opp : Side} {rev : List Trade}
    {rid : Nat} {resting resting' : Order} {t : Trade} {suffix : List Nat}
    (w : WalkCarry src book opp rev)
    (hnone : w.openId = none)
    (hq : queueOf book opp = rid :: suffix)
    (hinv : BookInv book')
    (hqueue : queueOf book' opp = suffix)
    (hfo : ∀ id, findOrder book'.orders id =
      if id = rid then some resting' else findOrder book.orders id)
    (hfind : findOrder book.orders rid = some resting)
    (hact : resting.status.isActive = true)
    (hpass : t.passiveId = rid) :
    ∃ w' : WalkCarry src book' opp (t :: rev), w'.openId = none := by
  have hd0 : (queueOf src opp).Nodup := side_flat_nodup w.srcInv opp
  have hd : (w.pref ++ rid :: suffix).Nodup := by simpa [w.queue, hq] using hd0
  have hnot : rid ∉ w.pref := by
    intro hp
    exact pref_disj hd hp mem_cons_self
  have hsub := (w.clearSub hnone).1
  refine ⟨{
    pref := w.pref ++ [rid]
    inv := hinv
    srcInv := w.srcInv
    queue := by
      have hsrc : queueOf src opp = w.pref ++ rid :: suffix := by simpa [hq] using w.queue
      simpa [hqueue, append_assoc] using hsrc
    openId := none
    clearSub := by
      intro _
      refine ⟨?_, ?_⟩
      · have hrev : ((t :: rev).map (·.passiveId)).reverse =
            (rev.map (·.passiveId)).reverse ++ [rid] := by simp [hpass, reverse_cons]
        rw [hrev]
        exact hsub.append (Sublist.refl [rid])
      · intro id hhead
        have hid : id ∈ suffix := by
          rw [hqueue] at hhead
          exact mem_of_head? hhead
        intro hin
        simp [hpass] at hin
        rcases hin with rfl | hin
        · exact (nodup_cons.mp (nodup_append.mp hd).2.1).1 hid
        · have hinP : id ∈ w.pref := hsub.subset (by simpa [mem_reverse] using hin)
          exact pref_disj hd hinP (mem_cons_of_mem rid hid)
    openSub := by
      intro _ hi
      cases hi
    spent := by
      intro id hid
      rw [mem_append] at hid
      rcases hid with hid | hid
      · have hne : id ≠ rid := by
          intro heq
          exact hnot (heq ▸ hid)
        rcases w.spent id hid with hin | ⟨o, hf, hs, hr⟩
        · exact Or.inl (mem_cons_of_mem _ hin)
        · have hfo' : findOrder book'.orders id = findOrder book.orders id := by
            simp [hfo id, hne]
          exact Or.inr ⟨o, by rw [hfo', hf], hs, hr⟩
      · have heq : id = rid := mem_singleton.mp hid
        rw [heq]
        exact Or.inl (by simp [hpass])
    live := by
      intro id hid
      rw [mem_append] at hid
      rcases hid with hid | hid
      · exact w.live id hid
      · have heq : id = rid := mem_singleton.mp hid
        rw [heq]
        rcases w.fresh rid (by simp [hq]) with heq | hbad
        · rw [heq] at hfind
          exact ⟨resting, hfind, hact⟩
        · rw [hnone] at hbad
          cases hbad
    fresh := by
      intro id hid
      have hidS : id ∈ suffix := by simpa [hqueue] using hid
      have hne : id ≠ rid := by
        intro heq
        exact (nodup_cons.mp (nodup_append.mp hd).2.1).1 (heq ▸ hidS)
      have hidOld : id ∈ queueOf book opp := by simpa [hq] using mem_cons_of_mem rid hidS
      have hfo' : findOrder book'.orders id = findOrder book.orders id := by simp [hfo id, hne]
      rw [hfo']
      rcases w.fresh id hidOld with heq | hbad
      · exact Or.inl heq
      · rw [hnone] at hbad
        cases hbad
  }, rfl⟩

theorem walk_touch {src book book' : Book} {opp : Side} {rev : List Trade}
    {rid : Nat} {t : Trade} {suffix : List Nat}
    (w : WalkCarry src book opp rev)
    (hnone : w.openId = none)
    (hq : queueOf book opp = rid :: suffix)
    (hinv : BookInv book')
    (hqueue : queueOf book' opp = queueOf book opp)
    (hfo : ∀ id, findOrder book'.orders id =
      if id = rid then findOrder book'.orders rid else findOrder book.orders id)
    (hpass : t.passiveId = rid) :
    Nonempty (WalkCarry src book' opp (t :: rev)) := by
  have hd0 : (queueOf src opp).Nodup := side_flat_nodup w.srcInv opp
  have hd : (w.pref ++ rid :: suffix).Nodup := by simpa [w.queue, hq] using hd0
  exact ⟨{
    pref := w.pref
    inv := hinv
    srcInv := w.srcInv
    queue := by simpa [hqueue] using w.queue
    openId := some rid
    clearSub := by
      intro h
      cases h
    openSub := by
      intro i hi
      cases hi
      refine ⟨?_, (rev.map (·.passiveId)).reverse, ?_, (w.clearSub hnone).1⟩
      · simp [hqueue, hq]
      · simp [hpass, reverse_cons]
    spent := by
      intro id hid
      have hne : id ≠ rid := by
        intro heq
        exact pref_disj hd (heq ▸ hid) mem_cons_self
      rcases w.spent id hid with hin | ⟨o, hf, hs, hr⟩
      · exact Or.inl (mem_cons_of_mem _ hin)
      · have hfo' : findOrder book'.orders id = findOrder book.orders id := by
          simp [hfo id, hne]
        exact Or.inr ⟨o, by rw [hfo', hf], hs, hr⟩
    live := w.live
    fresh := by
      intro id hid
      by_cases hie : id = rid
      · exact Or.inr (by simp [hie])
      · have hidOld : id ∈ queueOf book opp := by simpa [hqueue] using hid
        have hfo' : findOrder book'.orders id = findOrder book.orders id := by
          simp [hfo id, hie]
        rw [hfo']
        rcases w.fresh id hidOld with heq | hbad
        · exact Or.inl heq
        · rw [hnone] at hbad
          cases hbad
  }⟩

theorem walk_trade (policy : StpPolicy) {src book : Book} {opp : Side} {rev : List Trade}
    {incoming : Order} {px : Int} {rid n : Nat} {resting : Order}
    {rest : List Nat} {tail : List Level}
    (w : WalkCarry src book opp rev)
    (hnone : w.openId = none)
    (hopp : incoming.side.opposite = opp)
    (hsucc : incoming.remaining = Nat.succ n)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (_hside : resting.side = incoming.side.opposite) (_hpx : resting.price = px)
    (hact : resting.status.isActive = true) (hrest : 0 < resting.remaining)
    (hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hconf : stpConflict incoming resting policy = false) :
    StepOut src book opp rev incoming policy false := by
  let oppS := incoming.side.opposite
  let qty := min incoming.remaining resting.remaining
  let emitted := emitTrade book px qty incoming.id rid incoming.side
  let incoming' := fill incoming qty
  let resting' := fill resting qty
  let book1 := writeOrder emitted.1 resting'
  let book2 := if resting'.remaining == 0 then dequeue book1 oppS px rid else book1
  have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
  have hs0 : step book incoming policy false =
      { book := book2, incoming := incoming', stp := false, trade := some emitted.2,
        go := true } := by
    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty, emitted,
      incoming', resting', book1, book2, oppS]
  have hpass : emitted.2.passiveId = rid := by simp [emitted, emitTrade]
  have hidr : resting.id = rid := findOrder_id w.inv.keys hfind
  have hid' : resting'.id = rid := by simp [resting', fill, hidr]
  have hshapeO : levelsOf book opp =
      { price := px, orders := rid :: rest } :: tail := by simpa [hopp] using hshape
  let suffix := rest ++ tail.flatMap (·.orders)
  have hq : queueOf book opp = rid :: suffix := by simpa [suffix] using queue_front hshapeO
  cases hdead : (resting'.remaining == 0)
  case false =>
    have hbook : book2 = book1 := by simp [book2, hdead]
    have hs : step book incoming policy false =
        { book := book1, incoming := incoming', stp := false, trade := some emitted.2,
          go := true } := by simpa [hbook] using hs0
    have horders : book1.orders = upsert book.orders resting' := by
      simp [book1, writeOrder, emitted, orders_emit]
    have hfoStrong : ∀ id, findOrder book1.orders id =
        if id = rid then some resting' else findOrder book.orders id := by
      intro id
      rw [horders, findOrder_upsert]
      simp [hid']
    have hfo : ∀ id, findOrder book1.orders id =
        if id = rid then findOrder book1.orders rid else findOrder book.orders id := by
      intro id
      by_cases hie : id = rid
      · simp [hie, hfoStrong rid]
      · simp [hie, hfoStrong id]
    have hlev : levelsOf book1 opp = levelsOf book opp := by
      simp [book1, levels_write, emitted, levels_emit]
    have hqueue : queueOf book1 opp = queueOf book opp := by simp [queueOf, hlev]
    have hinv : BookInv book1 := by
      simpa [hs] using (step_progress w.inv incoming policy false).inv
    have hdead' :
        ((fill resting (min incoming.remaining resting.remaining)).remaining == 0) = false := by
      simpa [resting', qty] using hdead
    have hzero := incoming_filled incoming resting hdead'
    rcases walk_touch w hnone hq hinv hqueue hfo hpass with ⟨w'⟩
    unfold StepOut
    rw [hs]
    exact ⟨w', by simp [incoming', fill, hopp],
      fun _ => Or.inr (by simpa [incoming', qty] using hzero)⟩
  case true =>
    have hoppS : oppS = opp := hopp
    have hbook : book2 = dequeue book1 opp px rid := by simp [book2, hdead, hoppS]
    let bookD := dequeue book1 opp px rid
    have hs : step book incoming policy false =
        { book := bookD, incoming := incoming', stp := false, trade := some emitted.2,
          go := true } := by simpa [hbook] using hs0
    have horders : bookD.orders = upsert book.orders resting' := by
      simp [bookD, book1, orders_dequeue, writeOrder, emitted, orders_emit]
    have hfo : ∀ id, findOrder bookD.orders id =
        if id = rid then some resting' else findOrder book.orders id := by
      intro id
      rw [horders, findOrder_upsert]
      simp [hid']
    have hlev : levelsOf bookD opp = removeAt (levelsOf book opp) px rid := by
      simp [bookD, levels_dequeue, book1, levels_write, emitted, levels_emit]
    have hqueue : queueOf bookD opp = suffix := by
      rw [queueOf, hlev, hshapeO]
      simpa [suffix] using flat_drop_head px rid rest tail
    have hinv : BookInv bookD := by
      simpa [hs] using (step_progress w.inv incoming policy false).inv
    rcases walk_done w hnone hq hinv hqueue hfo hfind hact hpass with ⟨w', hopen'⟩
    unfold StepOut
    rw [hs]
    exact ⟨w', by simp [incoming', fill, hopp], fun _ => Or.inl hopen'⟩

theorem walk_step (policy : StpPolicy) {src book : Book} {opp : Side}
    {rev : List Trade} {incoming : Order} {stp : Bool}
    (w : WalkCarry src book opp rev)
    (hopp : incoming.side.opposite = opp)
    (hopen : w.openId = none ∨ incoming.remaining = 0) :
    StepOut src book opp rev incoming policy stp := by
  cases stp
  case true =>
    have hs : step book incoming policy true =
        { book, incoming, stp := true, trade := none, go := false } := by simp [step]
    exact walk_static w hs rfl hopp
  case false =>
    cases hsucc : incoming.remaining
    case zero =>
      have hs : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc]
      exact walk_static w hs rfl hopp
    case succ n =>
      cases hopen with
      | inr hzero =>
        rw [hsucc] at hzero
        cases hzero
      | inl hnone =>
        let oppS := incoming.side.opposite
        cases hbest : bestPrice (levelsOf book oppS)
        case none =>
          have hs : step book incoming policy false =
              { book, incoming, stp := false, trade := none, go := false } := by
            simp [step, hsucc, hbest, oppS]
          exact walk_static w hs rfl hopp
        case some px =>
          cases hcross : pricesCross incoming.side incoming.price px
          case false =>
            have hs : step book incoming policy false =
                { book, incoming, stp := false, trade := none, go := false } := by
              simp [step, hsucc, hbest, hcross, oppS]
            exact walk_static w hs rfl hopp
          case true =>
            rcases front_order w.inv oppS hbest with
              ⟨rid, resting, rest, tail, hshape, hfront, hfind, hside, hpx, hact, hrest⟩
            have hoppS : oppS = opp := hopp
            have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
            cases policy
            case off =>
              exact walk_trade .off w hnone hopp hsucc hbest hcross hfront hfind hside hpx
                hact hrest hshape (stpConflict_off incoming resting)
            case cancelNewest =>
              cases hconf : stpConflict incoming resting .cancelNewest
              case false =>
                exact walk_trade .cancelNewest w hnone hopp hsucc hbest hcross hfront hfind
                  hside hpx hact hrest hshape hconf
              case true =>
                have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
                have hs : step book incoming .cancelNewest false =
                    { book, incoming := stopIncoming incoming, stp := true, trade := none,
                      go := false } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq, oppS]
                exact walk_static w hs (by simp [stopIncoming]) hopp
            case cancelOldest =>
              cases hconf : stpConflict incoming resting .cancelOldest
              case false =>
                exact walk_trade .cancelOldest w hnone hopp hsucc hbest hcross hfront hfind
                  hside hpx hact hrest hshape hconf
              case true =>
                have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
                have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
                have hs : step book incoming .cancelOldest false =
                    { book := cancelOrder book rid, incoming, stp := false, trade := none,
                      go := true } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hsame, oppS]
                have hshapeO : levelsOf book opp =
                    { price := px, orders := rid :: rest } :: tail := by
                  simpa [hoppS] using hshape
                have hsideO : resting.side = opp := by simpa [hoppS] using hside
                rcases walk_cancel w hnone hshapeO hfind hsideO hpx hact with ⟨w', hopen'⟩
                unfold StepOut
                rw [hs]
                exact ⟨w', by simp [hopp], fun _ => Or.inl hopen'⟩
            case decrementAndCancel =>
              cases hconf : stpConflict incoming resting .decrementAndCancel
              case false =>
                exact walk_trade .decrementAndCancel w hnone hopp hsucc hbest hcross hfront
                  hfind hside hpx hact hrest hshape hconf
              case true =>
                have hnotNew :
                    (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
                have hnotOld :
                    (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
                have hdec :
                    (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
                by_cases hlt : n + 1 < resting.remaining
                · have hs : step book incoming .decrementAndCancel false =
                      { book, incoming := stopIncoming incoming, stp := true, trade := none,
                        go := false } := by
                    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                      hnotOld, hdec, hlt, oppS]
                  exact walk_static w hs (by simp [stopIncoming]) hopp
                · have hs : step book incoming .decrementAndCancel false =
                      { book := cancelOrder book rid, incoming, stp := false, trade := none,
                        go := true } := by
                    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                      hnotOld, hdec, hlt, oppS]
                  have hshapeO : levelsOf book opp =
                      { price := px, orders := rid :: rest } :: tail := by
                    simpa [hoppS] using hshape
                  have hsideO : resting.side = opp := by simpa [hoppS] using hside
                  rcases walk_cancel w hnone hshapeO hfind hsideO hpx hact with ⟨w', hopen'⟩
                  unfold StepOut
                  rw [hs]
                  exact ⟨w', by simp [hopp], fun _ => Or.inl hopen'⟩

theorem matchLoop_stop {n : Nat} {book : Book} {incoming : Order} {policy : StpPolicy}
    {rev : List Trade} {stp : Bool}
    (hgo : (step book incoming policy stp).go = false) :
    matchLoop (n + 1) book incoming policy rev stp =
      ((step book incoming policy stp).book, (step book incoming policy stp).incoming,
        (consTrade rev (step book incoming policy stp).trade).reverse,
        (step book incoming policy stp).stp) := by
  cases htr : (step book incoming policy stp).trade <;>
    simp [matchLoop, consTrade, hgo, htr]

theorem matchLoop_go {n : Nat} {book : Book} {incoming : Order} {policy : StpPolicy}
    {rev : List Trade} {stp : Bool}
    (hgo : (step book incoming policy stp).go = true) :
    matchLoop (n + 1) book incoming policy rev stp =
      matchLoop n (step book incoming policy stp).book (step book incoming policy stp).incoming
        policy (consTrade rev (step book incoming policy stp).trade)
        (step book incoming policy stp).stp := by
  cases htr : (step book incoming policy stp).trade <;>
    simp [matchLoop, consTrade, hgo, htr]

theorem walk_out (fuel : Nat) (policy : StpPolicy) (src : Book) (opp : Side)
    (book : Book) (incoming : Order) (rev : List Trade) (stp : Bool)
    (w : WalkCarry src book opp rev)
    (hopp : incoming.side.opposite = opp)
    (hopen : w.openId = none ∨ incoming.remaining = 0) :
    Priority src (matchLoop fuel book incoming policy rev stp).1 opp
      (matchLoop fuel book incoming policy rev stp).2.2.1 := by
  induction fuel generalizing book incoming rev stp w hopp hopen with
  | zero =>
    simpa [matchLoop] using walk_says w
  | succ _ ih =>
    rcases walk_step (stp := stp) policy w hopp hopen with ⟨w', hside, hcont⟩
    cases hgo : (step book incoming policy stp).go
    · rw [matchLoop_stop hgo]
      exact walk_says w'
    · rw [matchLoop_go hgo]
      exact ih (step book incoming policy stp).book (step book incoming policy stp).incoming
        (consTrade rev (step book incoming policy stp).trade)
        (step book incoming policy stp).stp w' hside (hcont hgo)

theorem walk_alloc (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) :
    let src := (alloc book side price quantity tif owner).1
    ∃ w : WalkCarry src src side.opposite [], w.openId = none := by
  have hinv : BookInv (alloc book side price quantity tif owner).1 :=
    (bookInv_alloc h side price quantity tif owner).1
  refine ⟨{
    pref := []
    inv := hinv
    srcInv := hinv
    queue := by simp [queueOf]
    openId := none
    clearSub := by
      intro _
      refine ⟨Sublist.refl [], ?_⟩
      intro _ _ hbad
      cases hbad
    openSub := by
      intro _ hi
      cases hi
    spent := by
      intro _ hid
      cases hid
    live := by
      intro _ hid
      cases hid
    fresh := by
      intro _ _
      exact Or.inl rfl
  }, rfl⟩

theorem submit_priority (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) (policy : StpPolicy) :
    ((submit book side price quantity tif owner policy).2.map (·.passiveId))
        <+ queueOf book side.opposite ∧
      ∀ id, skippedBeforeLast (queueOf book side.opposite)
          ((submit book side price quantity tif owner policy).2.map (·.passiveId)) id →
        ∃ before after,
          findOrder book.orders id = some before ∧
          before.status.isActive = true ∧
          findOrder (submit book side price quantity tif owner policy).1.orders id = some after ∧
          after.status = .cancelled ∧
          after.remaining = 0 := by
  unfold submit
  split
  · refine ⟨nil_sublist _, ?_⟩
    intro _ hsk
    rcases hsk with ⟨_, hlast, _⟩
    simp at hlast
  · let out := alloc book side price quantity tif owner
    let looped := matchLoop (matchFuel out.1 out.2) out.1 out.2 policy [] false
    have hpack := bookInv_alloc h side price quantity tif owner
    have horders : out.1.orders = book.orders := by simp [out, alloc]
    have hqueue : queueOf out.1 side.opposite = queueOf book side.opposite := by
      cases side <;> simp [queueOf, out, alloc, levelsOf, Side.opposite]
    rcases walk_alloc h side price quantity tif owner with ⟨w0, hopen0⟩
    have hloop := walk_out (matchFuel out.1 out.2) policy out.1 side.opposite
      out.1 out.2 [] false w0 (by simp [out, alloc]) (Or.inl hopen0)
    rcases hloop with ⟨hsub, hskip⟩
    refine ⟨?_, ?_⟩
    · simpa [out, looped, hqueue] using hsub
    · intro id hsk
      have hsk' : skippedBeforeLast (queueOf out.1 side.opposite)
          (looped.2.2.1.map (·.passiveId)) id := by
        simpa [out, looped, hqueue] using hsk
      rcases hskip id hsk' with ⟨before, after, hb, hact, hf, hst, hrem⟩
      have hbefore : findOrder book.orders id = some before := by simpa [horders] using hb
      have hmem : id ∈ queueOf book side.opposite := by
        rcases hsk with ⟨_, _, happears, _⟩
        exact mem_appears happears
      have hlt := queued_below h side.opposite hmem
      have hkeep := matchLoop_keep (matchFuel out.1 out.2) hpack.1 out.2 policy [] false
        hpack.2.2.1 hpack.2.2.2
      have hid : looped.2.1.id = book.nextOrderId := by
        have hidA : out.2.id = book.nextOrderId := by simp [out, alloc]
        have hidL : looped.2.1.id = out.2.id := by simpa [looped] using hkeep.2.1
        rw [hidL, hidA]
      have hne : id ≠ looped.2.1.id := by
        rw [hid]
        omega
      have hset :=
        find_settle_of_ne (book := looped.1) (o := looped.2.1) (stp := looped.2.2.2) hne
      refine ⟨before, after, hbefore, hact, ?_, hst, hrem⟩
      simpa [out, looped, hset] using hf

theorem law4_holds : law4 := by
  intro book hReach event view book' trades hview happ
  cases event with
  | cancel _ =>
    simp [eventSubmit] at hview
  | submitLimit side price quantity tif owner policy =>
    simp only [eventSubmit] at hview
    injection hview with hview
    cases hview
    have hf := submit_priority (reachable_inv hReach) side price quantity tif owner policy
    have hpair : submit book side price quantity tif owner policy = (book', trades) := happ
    simpa [hpair, queueOf] using hf
  | submitMarket side quantity owner policy =>
    cases side with
    | buy =>
      simp only [eventSubmit] at hview
      injection hview with hview
      cases hview
      have hf := submit_priority (reachable_inv hReach) .buy i64Max quantity .ioc owner policy
      have hpair :
          submit book .buy i64Max quantity .ioc owner policy = (book', trades) := happ
      simpa [hpair, queueOf] using hf
    | sell =>
      simp only [eventSubmit] at hview
      injection hview with hview
      cases hview
      have hf := submit_priority (reachable_inv hReach) .sell i64Min quantity .ioc owner policy
      have hpair :
          submit book .sell i64Min quantity .ioc owner policy = (book', trades) := happ
      simpa [hpair, queueOf] using hf

end Nanobook
