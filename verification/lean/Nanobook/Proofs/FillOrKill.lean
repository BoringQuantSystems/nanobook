/-
A fill-or-kill order trades its whole quantity, or the book does not change.
-/

import Nanobook.Proofs.Conservation

namespace Nanobook

open List

theorem beq_false_of_ne {a b : Nat} (h : a ≠ b) : (a == b) = false := by
  cases hb : (a == b)
  · rfl
  · exact absurd (by simpa using hb) h

theorem decide_false_of_not {p : Prop} [Decidable p] (h : ¬ p) : decide p = false := by
  cases hd : decide p
  · rfl
  · exact absurd (decide_eq_true_iff.mp hd) h

theorem ownerConflict_stp (incoming resting : Order) (policy : StpPolicy) :
    ownerConflict incoming.owner resting.owner policy = stpConflict incoming resting policy := by
  rfl

theorem scanIds_le (orders : List (Nat × Order)) (ids : List Nat) (r : Nat)
    (owner : Option Nat) (policy : StpPolicy) :
    (scanIds orders ids r owner policy).taken ≤ r := by
  induction ids generalizing r with
  | nil => simp [scanIds]
  | cons id rest ih =>
    unfold scanIds
    split
    · exact Nat.zero_le _
    · split
      · exact ih r
      · rename_i resting _hfind
        split
        · exact ih r
        · split
          · exact Nat.zero_le _
          · split
            · exact ih r
            · split
              · exact Nat.zero_le _
              · split
                · exact ih r
                · dsimp only
                  have htail := ih (r - min r resting.remaining)
                  omega

theorem scanIds_halt (orders : List (Nat × Order)) (ids : List Nat) (r : Nat)
    (owner : Option Nat) (policy : StpPolicy) :
    (scanIds orders ids r owner policy).halt = true →
      (scanIds orders ids r owner policy).taken < r := by
  induction ids generalizing r with
  | nil =>
    intro hh
    simp [scanIds] at hh
  | cons id rest ih =>
    unfold scanIds
    split
    · intro hh
      simp at hh
    · rename_i hr
      split
      · intro hh
        exact ih r hh
      · rename_i resting _hfind
        split
        · intro hh
          exact ih r hh
        · split
          · intro _
            have hpos : 0 < r := by
              cases r with
              | zero => simp at hr
              | succ _ => exact Nat.succ_pos _
            exact hpos
          · split
            · intro hh
              exact ih r hh
            · split
              · intro _
                have hpos : 0 < r := by
                  cases r with
                  | zero => simp at hr
                  | succ _ => exact Nat.succ_pos _
                exact hpos
              · split
                · intro hh
                  exact ih r hh
                · dsimp only
                  intro hh
                  have htail := ih (r - min r resting.remaining) hh
                  have hq : min r resting.remaining ≤ r := Nat.min_le_left _ _
                  have hsum := Nat.add_lt_add_left htail (min r resting.remaining)
                  rwa [Nat.add_sub_of_le hq] at hsum

theorem scanLevels_le (orders : List (Nat × Order)) (levels : List Level) (side : Side)
    (price : Int) (r : Nat) (owner : Option Nat) (policy : StpPolicy) :
    (scanLevels orders levels side price r owner policy).taken ≤ r := by
  induction levels generalizing r with
  | nil => simp [scanLevels]
  | cons level rest ih =>
    unfold scanLevels
    dsimp only
    split
    · exact Nat.zero_le _
    · split
      · split
        · exact scanIds_le orders level.orders r owner policy
        · split
          · exact scanIds_le orders level.orders r owner policy
          · dsimp only
            have hhere := scanIds_le orders level.orders r owner policy
            have hmore := ih (r - (scanIds orders level.orders r owner policy).taken)
            omega
      · exact Nat.zero_le _

theorem scanLevels_halt (orders : List (Nat × Order)) (levels : List Level) (side : Side)
    (price : Int) (r : Nat) (owner : Option Nat) (policy : StpPolicy) :
    (scanLevels orders levels side price r owner policy).halt = true →
      (scanLevels orders levels side price r owner policy).taken < r := by
  induction levels generalizing r with
  | nil =>
    intro hh
    simp [scanLevels] at hh
  | cons level rest ih =>
    unfold scanLevels
    dsimp only
    split
    · intro hh
      simp at hh
    · split
      · split
        · intro hh
          exact scanIds_halt orders level.orders r owner policy hh
        · rename_i hnh
          split
          · intro hh
            exact absurd hh hnh
          · dsimp only
            intro hh
            have hhere := scanIds_le orders level.orders r owner policy
            have hmore := ih (r - (scanIds orders level.orders r owner policy).taken) hh
            have hsum := Nat.add_lt_add_left hmore
              (scanIds orders level.orders r owner policy).taken
            rwa [Nat.add_sub_of_le hhere] at hsum
      · intro hh
        simp at hh

theorem scanLevels_zero (orders : List (Nat × Order)) (levels : List Level) (side : Side)
    (price : Int) (owner : Option Nat) (policy : StpPolicy) :
    scanLevels orders levels side price 0 owner policy = { taken := 0, halt := false } := by
  cases levels <;> simp [scanLevels]

theorem available_le (book : Book) (side : Side) (price : Int) (quantity : Nat)
    (owner : Option Nat) (policy : StpPolicy) :
    available book side price quantity owner policy ≤ quantity := by
  simpa [available, sumCrossing] using
    scanLevels_le book.orders (levelsOf book side.opposite) side price quantity owner policy

theorem scanIds_congr
    {orders orders' : List (Nat × Order)} {ids : List Nat}
    {r : Nat} {owner : Option Nat} {policy : StpPolicy}
    (h : ∀ id ∈ ids, findOrder orders' id = findOrder orders id) :
    scanIds orders' ids r owner policy = scanIds orders ids r owner policy := by
  induction ids generalizing r with
  | nil => simp [scanIds]
  | cons id rest ih =>
    have hhead := h id mem_cons_self
    have htail : ∀ i ∈ rest, findOrder orders' i = findOrder orders i :=
      fun i hi => h i (mem_cons_of_mem _ hi)
    rw [scanIds.eq_2, scanIds.eq_2]
    split
    · rfl
    · rw [hhead]
      split
      · exact ih (r := r) htail
      · rename_i resting _hfind
        split
        · exact ih (r := r) htail
        · split
          · rfl
          · split
            · exact ih (r := r) htail
            · split
              · rfl
              · split
                · exact ih (r := r) htail
                · dsimp only
                  rw [ih (r := r - min r resting.remaining) htail]

theorem scanLevels_congr
    {orders orders' : List (Nat × Order)} {levels : List Level}
    {side : Side} {price : Int} {r : Nat} {owner : Option Nat} {policy : StpPolicy}
    (h : ∀ id ∈ levels.flatMap (·.orders), findOrder orders' id = findOrder orders id) :
    scanLevels orders' levels side price r owner policy =
      scanLevels orders levels side price r owner policy := by
  induction levels generalizing r with
  | nil => simp [scanLevels]
  | cons level rest ih =>
    have hlevel : ∀ id ∈ level.orders, findOrder orders' id = findOrder orders id := by
      intro id hid
      exact h id (mem_append_left _ hid)
    have hrest : ∀ id ∈ rest.flatMap (·.orders), findOrder orders' id = findOrder orders id := by
      intro id hid
      exact h id (mem_append_right _ hid)
    rw [scanLevels.eq_2, scanLevels.eq_2]
    dsimp only
    split
    · rfl
    · split
      · rw [scanIds_congr hlevel]
        split
        · rfl
        · split
          · rfl
          · rw [ih (r := r - (scanIds orders level.orders r owner policy).taken) hrest]
      · rfl

theorem removeAt_front (px : Int) (rid : Nat) (rest : List Nat) (tail : List Level) :
    removeAt ({ price := px, orders := rid :: rest } :: tail) px rid =
      if rest = [] then tail else { price := px, orders := rest } :: tail := by
  cases rest with
  | nil => simp [removeAt, removeId]
  | cons a as => simp [removeAt, removeId]

theorem queue_head_unique (h : BookInv book) {side : Side} {px : Int} {rid : Nat}
    {rest : List Nat} {tail : List Level}
    (hshape : levelsOf book side = { price := px, orders := rid :: rest } :: tail) :
    (∀ id ∈ rest, id ≠ rid) ∧ (∀ id ∈ tail.flatMap (·.orders), id ≠ rid) := by
  have hd := side_flat_nodup h side
  have hd' : (rid :: (rest ++ tail.flatMap (·.orders))).Nodup := by
    rw [hshape] at hd
    simpa [List.flatMap] using hd
  have hnin : rid ∉ rest ++ tail.flatMap (·.orders) := (nodup_cons.mp hd').1
  refine ⟨?_, ?_⟩
  · intro id hid heq
    exact hnin (heq ▸ mem_append_left _ hid)
  · intro id hid heq
    exact hnin (heq ▸ mem_append_right _ hid)

theorem scanIds_newest
    {orders : List (Nat × Order)} {id : Nat} {ids : List Nat} {r : Nat}
    {resting : Order} {owner : Option Nat}
    (hr0 : (r == 0) = false)
    (hfind : findOrder orders id = some resting)
    (hact : resting.status.isActive = true)
    (hzero : (resting.remaining == 0) = false)
    (hown : ownerConflict owner resting.owner .cancelNewest = true) :
    scanIds orders (id :: ids) r owner .cancelNewest = { taken := 0, halt := true } := by
  have hpol : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
  rw [scanIds.eq_2]
  simp only [hr0, hfind, hact, hzero, hown, hpol,
    Bool.not_true, Bool.or_false, Bool.and_true, ↓reduceIte, ↓reduceCtorEq]

theorem scanIds_oldest
    {orders : List (Nat × Order)} {id : Nat} {ids : List Nat} {r : Nat}
    {resting : Order} {owner : Option Nat} {policy : StpPolicy}
    (hr0 : (r == 0) = false)
    (hfind : findOrder orders id = some resting)
    (hact : resting.status.isActive = true)
    (hzero : (resting.remaining == 0) = false)
    (hown : ownerConflict owner resting.owner policy = true)
    (hnew : (policy == .cancelNewest) = false)
    (hold : (policy == .cancelOldest) = true) :
    scanIds orders (id :: ids) r owner policy = scanIds orders ids r owner policy := by
  rw [scanIds.eq_2]
  simp only [hr0, hfind, hact, hzero, hown, hnew, hold,
    Bool.not_true, Bool.or_false, Bool.and_true, Bool.and_false,
    ↓reduceIte, ↓reduceCtorEq]

theorem scanIds_decHalt
    {orders : List (Nat × Order)} {id : Nat} {ids : List Nat} {r : Nat}
    {resting : Order} {owner : Option Nat}
    (hr0 : (r == 0) = false)
    (hfind : findOrder orders id = some resting)
    (hact : resting.status.isActive = true)
    (hzero : (resting.remaining == 0) = false)
    (hown : ownerConflict owner resting.owner .decrementAndCancel = true)
    (hlt : decide (r < resting.remaining) = true) :
    scanIds orders (id :: ids) r owner .decrementAndCancel = { taken := 0, halt := true } := by
  have hnew : (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
  have hold : (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
  have hdec : (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
  rw [scanIds.eq_2]
  simp only [hr0, hfind, hact, hzero, hown, hnew, hold, hdec, hlt,
    Bool.not_true, Bool.or_false, Bool.and_true, Bool.and_false,
    ↓reduceIte, ↓reduceCtorEq]

theorem scanIds_decSkip
    {orders : List (Nat × Order)} {id : Nat} {ids : List Nat} {r : Nat}
    {resting : Order} {owner : Option Nat}
    (hr0 : (r == 0) = false)
    (hfind : findOrder orders id = some resting)
    (hact : resting.status.isActive = true)
    (hzero : (resting.remaining == 0) = false)
    (hown : ownerConflict owner resting.owner .decrementAndCancel = true)
    (hlt : decide (r < resting.remaining) = false) :
    scanIds orders (id :: ids) r owner .decrementAndCancel =
      scanIds orders ids r owner .decrementAndCancel := by
  have hnew : (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
  have hold : (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
  have hdec : (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
  rw [scanIds.eq_2]
  simp only [hr0, hfind, hact, hzero, hown, hnew, hold, hdec, hlt,
    Bool.not_true, Bool.or_false, Bool.and_true, Bool.and_false,
    ↓reduceIte, ↓reduceCtorEq]

theorem scanIds_trade
    {orders : List (Nat × Order)} {id : Nat} {ids : List Nat} {r : Nat}
    {resting : Order} {owner : Option Nat} {policy : StpPolicy}
    (hr0 : (r == 0) = false)
    (hfind : findOrder orders id = some resting)
    (hact : resting.status.isActive = true)
    (hzero : (resting.remaining == 0) = false)
    (hno : ownerConflict owner resting.owner policy = false) :
    scanIds orders (id :: ids) r owner policy =
      { taken := min r resting.remaining +
          (scanIds orders ids (r - min r resting.remaining) owner policy).taken
      , halt := (scanIds orders ids (r - min r resting.remaining) owner policy).halt } := by
  rw [scanIds.eq_2]
  simp only [hr0, hfind, hact, hzero, hno,
    Bool.not_true, Bool.or_false, Bool.false_and, ↓reduceIte, ↓reduceCtorEq]

theorem scanLevels_stopped
    {orders : List (Nat × Order)} {rid : Nat} {rest : List Nat} {tail : List Level}
    {side : Side} {price px : Int} {r : Nat} {owner : Option Nat} {policy : StpPolicy}
    (hr0 : (r == 0) = false)
    (hcross : pricesCross side price px = true)
    (hhere : scanIds orders (rid :: rest) r owner policy = { taken := 0, halt := true }) :
    (scanLevels orders ({ price := px, orders := rid :: rest } :: tail)
      side price r owner policy).taken = 0 := by
  rw [scanLevels.eq_2]
  simp only [hr0, hcross, hhere, ↓reduceIte, ↓reduceCtorEq]

/-- One full step keeps a complete fill in reach. -/
structure StepFull (book : Book) (incoming : Order) (policy : StpPolicy) : Prop where
  go : (step book incoming policy false).go = true
  stp : (step book incoming policy false).stp = false
  inv : BookInv (step book incoming policy false).book
  fuel : matchFuel (step book incoming policy false).book
      (step book incoming policy false).incoming < matchFuel book incoming
  scan : (scanLevels (step book incoming policy false).book.orders
      (levelsOf (step book incoming policy false).book
        (step book incoming policy false).incoming.side.opposite)
      (step book incoming policy false).incoming.side
      (step book incoming policy false).incoming.price
      (step book incoming policy false).incoming.remaining
      (step book incoming policy false).incoming.owner policy).taken =
    (step book incoming policy false).incoming.remaining

theorem scan_drop_head
    {orders orders' : List (Nat × Order)}
    {rid : Nat} {rest : List Nat} {tail : List Level}
    {side : Side} {price px : Int} {r : Nat}
    {owner : Option Nat} {policy : StpPolicy}
    (hcross : pricesCross side price px = true)
    (hpos : 0 < r)
    (hskip : scanIds orders (rid :: rest) r owner policy = scanIds orders rest r owner policy)
    (hrest : ∀ id ∈ rest, id ≠ rid)
    (htail : ∀ id ∈ tail.flatMap (·.orders), id ≠ rid)
    (hagree : ∀ id, id ≠ rid → findOrder orders' id = findOrder orders id) :
    scanLevels orders' (removeAt ({ price := px, orders := rid :: rest } :: tail) px rid)
        side price r owner policy =
      scanLevels orders ({ price := px, orders := rid :: rest } :: tail)
        side price r owner policy := by
  have hr0 : (r == 0) = false := beq_false_of_ne (Nat.ne_of_gt hpos)
  have hrestScan : scanIds orders' rest r owner policy = scanIds orders rest r owner policy :=
    scanIds_congr (fun id hid => hagree id (hrest id hid))
  have htailScan : ∀ r', scanLevels orders' tail side price r' owner policy =
      scanLevels orders tail side price r' owner policy := by
    intro r'
    exact scanLevels_congr (fun id hid => hagree id (htail id hid))
  rw [removeAt_front]
  by_cases hempty : rest = []
  · subst hempty
    simp only [↓reduceIte]
    rw [htailScan r, scanLevels.eq_2]
    have hzeroTaken : ((0 : Nat) == r) = false := beq_false_of_ne (Nat.ne_of_lt hpos)
    simp only [hr0, hcross, hskip, scanIds.eq_1, hzeroTaken, Nat.sub_zero,
      ↓reduceIte, ↓reduceCtorEq]
    simp [Nat.zero_add]
  · simp only [hempty, ↓reduceIte]
    rw [scanLevels.eq_2, scanLevels.eq_2]
    simp only [hr0, hcross, hskip, hrestScan, ↓reduceIte]
    rw [htailScan (r - (scanIds orders rest r owner policy).taken)]

theorem scan_take_head
    {orders orders' : List (Nat × Order)}
    {rid : Nat} {rest : List Nat} {tail : List Level} {resting : Order}
    {side : Side} {price px : Int} {r : Nat}
    {owner : Option Nat} {policy : StpPolicy}
    (hcross : pricesCross side price px = true)
    (hpos : 0 < r)
    (hrest : 0 < resting.remaining)
    (hqlt : resting.remaining < r)
    (hfind : findOrder orders rid = some resting)
    (hact : resting.status.isActive = true)
    (hno : ownerConflict owner resting.owner policy = false)
    (hrestNe : ∀ id ∈ rest, id ≠ rid)
    (htailNe : ∀ id ∈ tail.flatMap (·.orders), id ≠ rid)
    (hagree : ∀ id, id ≠ rid → findOrder orders' id = findOrder orders id)
    (hfull : (scanLevels orders ({ price := px, orders := rid :: rest } :: tail)
      side price r owner policy).taken = r) :
    (scanLevels orders' (removeAt ({ price := px, orders := rid :: rest } :: tail) px rid)
      side price (r - resting.remaining) owner policy).taken =
      r - resting.remaining := by
  have hr0 : (r == 0) = false := beq_false_of_ne (Nat.ne_of_gt hpos)
  have hzero : (resting.remaining == 0) = false := beq_false_of_ne (Nat.ne_of_gt hrest)
  have hq : min r resting.remaining = resting.remaining := Nat.min_eq_right (Nat.le_of_lt hqlt)
  have hIds := scanIds_trade (id := rid) (ids := rest) hr0 hfind hact hzero hno
  rw [hq] at hIds
  let tailS := scanIds orders rest (r - resting.remaining) owner policy
  have hIds' : scanIds orders (rid :: rest) r owner policy =
      { taken := resting.remaining + tailS.taken, halt := tailS.halt } := by
    simpa [tailS] using hIds
  have hnh : tailS.halt = false := by
    cases hh : tailS.halt
    · rfl
    · have hlt := scanIds_halt orders (rid :: rest) r owner policy (by simpa [hIds'] using hh)
      have hgot : (scanLevels orders ({ price := px, orders := rid :: rest } :: tail)
          side price r owner policy).taken = resting.remaining + tailS.taken := by
        rw [scanLevels.eq_2]
        simp [hr0, hcross, hIds', hh]
      have hlt' : resting.remaining + tailS.taken < r := by simpa [hIds'] using hlt
      have heqGot : resting.remaining + tailS.taken = r := by simpa [hgot] using hfull
      omega
  have htailIds : scanIds orders' rest (r - resting.remaining) owner policy = tailS := by
    simpa [tailS] using
      (scanIds_congr (fun id hid => hagree id (hrestNe id hid)) :
        scanIds orders' rest (r - resting.remaining) owner policy =
          scanIds orders rest (r - resting.remaining) owner policy)
  have htailLv : ∀ r', (scanLevels orders' tail side price r' owner policy).taken =
      (scanLevels orders tail side price r' owner policy).taken := by
    intro r'
    rw [scanLevels_congr (fun id hid => hagree id (htailNe id hid))]
  rw [removeAt_front]
  by_cases hempty : rest = []
  · have htail0 : tailS = { taken := 0, halt := false } := by
      simp [tailS, hempty, scanIds]
    have hmore : (scanLevels orders tail side price (r - resting.remaining) owner policy).taken =
        r - resting.remaining := by
      have hne : ((resting.remaining : Nat) == r) = false := beq_false_of_ne (Nat.ne_of_lt hqlt)
      rw [scanLevels.eq_2] at hfull
      simp [hr0, hcross, hIds', htail0, hne, Nat.add_zero] at hfull
      omega
    simp [hempty, htailLv, hmore]
  · have hr' : (r - resting.remaining == 0) = false := by
      have hpos' : 0 < r - resting.remaining := Nat.sub_pos_of_lt hqlt
      exact beq_false_of_ne (Nat.ne_of_gt hpos')
    simp only [hempty, ↓reduceIte]
    rw [scanLevels.eq_2]
    simp [hr', hcross, htailIds, hnh]
    by_cases heq : resting.remaining + tailS.taken = r
    · have htake : tailS.taken = r - resting.remaining := by omega
      simp [htake]
    · have hle : resting.remaining + tailS.taken ≤ r := by
        have hleS := scanIds_le orders (rid :: rest) r owner policy
        simpa [hIds'] using hleS
      have hneq : (tailS.taken == r - resting.remaining) = false :=
        beq_false_of_ne (by
          intro hbad
          apply heq
          omega)
      have hne' : ¬ tailS.taken = r - resting.remaining := by
        intro hbad
        apply heq
        omega
      have hmore : (scanLevels orders tail side price
          (r - resting.remaining - tailS.taken) owner policy).taken =
          r - resting.remaining - tailS.taken := by
        have hsubIdx : r - (resting.remaining + tailS.taken) =
            r - resting.remaining - tailS.taken := by omega
        rw [scanLevels.eq_2] at hfull
        split at hfull
        · have hzeroR : 0 = r := hfull
          cases hzeroR
          exact absurd hpos (Nat.lt_irrefl 0)
        · dsimp only at hfull
          split at hfull
          · exact absurd (by simpa [hIds'] using hfull) heq
          · split at hfull
            · exact absurd (by simpa [hIds'] using hfull) heq
            · simp [hIds', hsubIdx] at hfull
              omega
      simp [htailLv, hmore, hne']
      omega

theorem step_full_skip (policy : StpPolicy) (hinv : BookInv book)
    {incoming resting : Order} {px : Int} {rid : Nat} {rest : List Nat} {tail : List Level}
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hpos : 0 < incoming.remaining)
    (hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hfind : findOrder book.orders rid = some resting)
    (hside : resting.side = incoming.side.opposite)
    (hpx : resting.price = px)
    (hact : resting.status.isActive = true)
    (hskip : scanIds book.orders (rid :: rest) incoming.remaining incoming.owner policy =
      scanIds book.orders rest incoming.remaining incoming.owner policy)
    (hs : step book incoming policy false =
      { book := cancelOrder book rid, incoming, stp := false, trade := none, go := true })
    (hscan : (scanLevels book.orders (levelsOf book incoming.side.opposite)
      incoming.side incoming.price incoming.remaining incoming.owner policy).taken =
      incoming.remaining) :
    StepFull book incoming policy := by
  let opp := incoming.side.opposite
  let o' : Order := { resting with remaining := 0, status := .cancelled }
  have hidr : resting.id = rid := findOrder_id hinv.keys hfind
  have hc : cancelOrder book rid = dequeue (writeOrder book o') opp px rid := by
    simpa [o', hside, hpx] using cancelOrder_active hfind hact
  have hlev : levelsOf (cancelOrder book rid) opp = removeAt (levelsOf book opp) px rid := by
    rw [hc]
    simp [levels_dequeue, levels_write]
  have horders : (cancelOrder book rid).orders = upsert book.orders o' := by
    rw [hc]
    simp [orders_dequeue, writeOrder]
  have hagree : ∀ id, id ≠ rid →
      findOrder (cancelOrder book rid).orders id = findOrder book.orders id := by
    intro id hne
    have hid' : o'.id = rid := by simp [o', hidr]
    rw [horders, findOrder_upsert]
    simp [hid', hne]
  have huniq := queue_head_unique hinv hshape
  have hdrop := scan_drop_head (orders' := upsert book.orders o') hcross hpos hskip huniq.1 huniq.2
    (by
      intro id hne
      simpa [horders] using hagree id hne)
  have hprog := step_progress hinv incoming policy false
  have hgo : (step book incoming policy false).go = true := by simp [hs]
  refine ⟨by simp [hs], by simp [hs], by simpa [hs] using hprog.inv,
    by simpa [hs] using hprog.fuel hgo, ?_⟩
  simp only [hs]
  rw [hlev, hshape, horders]
  rw [hdrop]
  simpa [hshape] using hscan

theorem step_full_trade (policy : StpPolicy) (hinv : BookInv book)
    {incoming resting : Order} {px : Int} {rid n : Nat} {rest : List Nat} {tail : List Level}
    (hsucc : incoming.remaining = Nat.succ n)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (hact : resting.status.isActive = true)
    (hrest : 0 < resting.remaining)
    (hconf : stpConflict incoming resting policy = false)
    (hscan : (scanLevels book.orders (levelsOf book incoming.side.opposite)
      incoming.side incoming.price incoming.remaining incoming.owner policy).taken =
      incoming.remaining) :
    StepFull book incoming policy := by
  let opp := incoming.side.opposite
  let qty := min incoming.remaining resting.remaining
  have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
  let emitted := emitTrade book px qty incoming.id rid incoming.side
  let incoming' := fill incoming qty
  let resting' := fill resting qty
  let book1 := writeOrder emitted.1 resting'
  let book2 := if resting'.remaining == 0 then dequeue book1 opp px rid else book1
  have hs : step book incoming policy false =
      { book := book2, incoming := incoming', stp := false, trade := some emitted.2, go := true } := by
    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty, emitted, incoming',
      resting', book1, book2, opp]
  have hprog := step_progress hinv incoming policy false
  have hgo : (step book incoming policy false).go = true := by simp [hs]
  refine ⟨by simp [hs], by simp [hs], by simpa [hs] using hprog.inv,
    by simpa [hs] using hprog.fuel hgo, ?_⟩
  simp only [hs]
  by_cases hle : incoming.remaining ≤ resting.remaining
  · have hqeq : qty = incoming.remaining := Nat.min_eq_left hle
    have hrem0 : incoming'.remaining = 0 := by simp [incoming', fill, hqeq]
    rw [hrem0]
    simp [scanLevels_zero]
  · have hqlt : resting.remaining < incoming.remaining := Nat.lt_of_not_le hle
    have hqeq : qty = resting.remaining := Nat.min_eq_right (Nat.le_of_lt hqlt)
    have hdead : resting'.remaining = 0 := by simp [resting', fill, hqeq]
    have hdeadB : (resting'.remaining == 0) = true := by simp [hdead]
    have hbook2 : book2 = dequeue book1 opp px rid := by simp [book2, hdeadB]
    have hlev : levelsOf book2 opp =
        removeAt ({ price := px, orders := rid :: rest } :: tail) px rid := by
      simp only [hbook2, levels_dequeue, book1, levels_write, emitted, levels_emit]
      rw [hshape]
    have horders : book2.orders = upsert book.orders resting' := by
      rw [hbook2]
      simp [book1, orders_dequeue, writeOrder, emitted, orders_emit]
    have hidr : resting.id = rid := findOrder_id hinv.keys hfind
    have hid' : resting'.id = rid := by simp [resting', fill, hidr]
    have hagree : ∀ id, id ≠ rid → findOrder book2.orders id = findOrder book.orders id := by
      intro id hneId
      rw [horders, findOrder_upsert]
      simp [hid', hneId]
    have hno : ownerConflict incoming.owner resting.owner policy = false := by
      rw [ownerConflict_stp]
      exact hconf
    have huniq := queue_head_unique hinv hshape
    have hpos : 0 < incoming.remaining := by simp [hsucc]
    have htake := scan_take_head (orders' := book2.orders) hcross hpos hrest hqlt hfind hact hno
      huniq.1 huniq.2 hagree (by simpa [hshape] using hscan)
    have hsideI : incoming'.side = incoming.side := by simp [incoming', fill]
    have hpriceI : incoming'.price = incoming.price := by simp [incoming', fill]
    have hownerI : incoming'.owner = incoming.owner := by simp [incoming', fill]
    have hremI : incoming'.remaining = incoming.remaining - resting.remaining := by
      simp [incoming', fill, hqeq]
    rw [hsideI, hpriceI, hownerI, hremI, hlev]
    exact htake

theorem step_full (policy : StpPolicy) (hinv : BookInv book) {incoming : Order}
    (hpos : 0 < incoming.remaining)
    (hscan : (scanLevels book.orders (levelsOf book incoming.side.opposite)
      incoming.side incoming.price incoming.remaining incoming.owner policy).taken =
      incoming.remaining) :
    StepFull book incoming policy := by
  have hr0 : (incoming.remaining == 0) = false := beq_false_of_ne (Nat.ne_of_gt hpos)
  cases hsucc : incoming.remaining with
  | zero =>
    exact absurd hsucc (Nat.ne_of_gt hpos)
  | succ n =>
    let opp := incoming.side.opposite
    cases hbest : bestPrice (levelsOf book opp) with
    | none =>
      exfalso
      have hempty : levelsOf book opp = [] := by
        cases hlevels : levelsOf book opp
        · rfl
        · simp [bestPrice, hlevels] at hbest
      rw [hempty] at hscan
      simp [scanLevels] at hscan
      omega
    | some px =>
      rcases front_order hinv opp hbest with
        ⟨rid, resting, rest, tail, hshape, hfront, hfind, hside, hpx, hact, hrest⟩
      have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
      have hzero : (resting.remaining == 0) = false := beq_false_of_ne hne
      cases hcross : pricesCross incoming.side incoming.price px with
      | false =>
        exfalso
        rw [hshape] at hscan
        unfold scanLevels at hscan
        simp [hr0, hcross] at hscan
        omega
      | true =>
        cases hconf : stpConflict incoming resting policy with
        | false =>
          exact step_full_trade policy hinv hsucc hbest hcross hshape hfront hfind hact hrest
            hconf hscan
        | true =>
          cases policy with
          | off =>
            have hoff := stpConflict_off incoming resting
            rw [hconf] at hoff
            cases hoff
          | cancelNewest =>
            exfalso
            have hown : ownerConflict incoming.owner resting.owner .cancelNewest = true := by
              rw [ownerConflict_stp]
              exact hconf
            have hhere := scanIds_newest (id := rid) (ids := rest) hr0 hfind hact hzero hown
            have hgot := scanLevels_stopped (tail := tail) hr0 hcross hhere
            rw [hshape] at hscan
            omega
          | cancelOldest =>
            have hown : ownerConflict incoming.owner resting.owner .cancelOldest = true := by
              rw [ownerConflict_stp]
              exact hconf
            have hnew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
            have hold : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
            have hskip := scanIds_oldest (id := rid) (ids := rest) hr0 hfind hact hzero hown hnew hold
            have hs : step book incoming .cancelOldest false =
                { book := cancelOrder book rid, incoming, stp := false, trade := none,
                  go := true } := by
              simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnew, hold, opp]
            exact step_full_skip .cancelOldest hinv hcross hpos hshape hfind hside hpx hact hskip
              hs hscan
          | decrementAndCancel =>
            have hown : ownerConflict incoming.owner resting.owner .decrementAndCancel = true := by
              rw [ownerConflict_stp]
              exact hconf
            have hnew : (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
            have hold : (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
            have hdec : (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
            by_cases hlt : incoming.remaining < resting.remaining
            · exfalso
              have hltB : decide (incoming.remaining < resting.remaining) = true :=
                decide_eq_true_iff.mpr hlt
              have hhere := scanIds_decHalt (id := rid) (ids := rest) hr0 hfind hact hzero hown hltB
              have hgot := scanLevels_stopped (tail := tail) hr0 hcross hhere
              rw [hshape] at hscan
              omega
            · have hltB : decide (incoming.remaining < resting.remaining) = false :=
                decide_false_of_not hlt
              have hskip := scanIds_decSkip (id := rid) (ids := rest) hr0 hfind hact hzero hown hltB
              have hnotN : ¬ n + 1 < resting.remaining := by simpa [hsucc] using hlt
              have hs : step book incoming .decrementAndCancel false =
                  { book := cancelOrder book rid, incoming, stp := false, trade := none,
                    go := true } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnew, hold,
                  hdec, hnotN, opp]
              exact step_full_skip .decrementAndCancel hinv hcross hpos hshape hfind hside hpx
                hact hskip hs hscan

theorem loop_fills (fuel : Nat) (policy : StpPolicy)
    {book : Book} {incoming : Order} {rev : List Trade}
    (hinv : BookInv book)
    (hfuel : matchFuel book incoming ≤ fuel)
    (hscan : (scanLevels book.orders (levelsOf book incoming.side.opposite)
      incoming.side incoming.price incoming.remaining incoming.owner policy).taken =
      incoming.remaining) :
    (matchLoop fuel book incoming policy rev false).2.1.remaining = 0 ∧
      (matchLoop fuel book incoming policy rev false).2.2.2 = false := by
  induction fuel generalizing book incoming rev with
  | zero =>
    have hmf := Nat.eq_zero_of_le_zero hfuel
    have hrem : incoming.remaining = 0 := by
      simp only [matchFuel] at hmf
      omega
    simp [matchLoop, hrem]
  | succ n ih =>
    cases hzero : incoming.remaining with
    | zero =>
      have hs : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hzero]
      simp [matchLoop, hs, hzero]
    | succ k =>
      have hpos : 0 < incoming.remaining := by simp [hzero]
      have hfull := step_full policy hinv hpos hscan
      have hle : matchFuel (step book incoming policy false).book
          (step book incoming policy false).incoming ≤ n := by
        have hlt := hfull.fuel
        omega
      simp only [matchLoop, hfull.go, hfull.stp]
      exact ih hfull.inv hle hfull.scan

theorem law7_holds : law7 := by
  intro book hreach side price quantity owner policy
  have hinv := reachable_inv hreach
  by_cases hlt : available book side price quantity owner policy < quantity
  · right
    have hdec : decide (available book side price quantity owner policy < quantity) = true :=
      decide_eq_true_iff.mpr hlt
    have htif : (TimeInForce.fok == .fok) = true := by decide
    simp [apply, submit, hdec, htif, alloc, ↓reduceIte]
  · left
    have hdec : decide (available book side price quantity owner policy < quantity) = false :=
      decide_false_of_not hlt
    have htif : (TimeInForce.fok == .fok) = true := by decide
    have heq : available book side price quantity owner policy = quantity := by
      have hle := available_le book side price quantity owner policy
      omega
    let out := alloc book side price quantity .fok owner
    let looped := matchLoop (matchFuel out.1 out.2) out.1 out.2 policy [] false
    have hinv1 : BookInv out.1 := (bookInv_alloc hinv side price quantity .fok owner).1
    have hscan1 : (scanLevels out.1.orders (levelsOf out.1 out.2.side.opposite)
        out.2.side out.2.price out.2.remaining out.2.owner policy).taken = out.2.remaining := by
      have horders : out.1.orders = book.orders := by simp [out, alloc]
      have hside : out.2.side = side := by simp [out, alloc]
      have hprice : out.2.price = price := by simp [out, alloc]
      have howner : out.2.owner = owner := by simp [out, alloc]
      have hrem : out.2.remaining = quantity := by simp [out, alloc]
      have hlevels : levelsOf out.1 side.opposite = levelsOf book side.opposite := by
        cases side <;> simp [out, alloc, levelsOf]
      rw [horders, hside, hprice, howner, hrem, hlevels]
      simpa [available, sumCrossing] using heq
    have hloop := loop_fills (rev := []) (matchFuel out.1 out.2) policy hinv1
      (Nat.le_refl _) hscan1
    have hacc : MatchAcc book quantity side price out.1 out.2 [] false := by
      simpa [out] using alloc_acc hinv side price quantity .fok owner
    have hmatch := matchLoop_acc (matchFuel out.1 out.2) policy hacc
    have hfilled : looped.2.1.filled = quantity := by
      have hl := hmatch.live hloop.2
      rw [hloop.1] at hl
      simpa using hl
    have hsum : tradeQtySum looped.2.2.1 = quantity := by
      rw [hmatch.sumEq, hfilled]
    simpa [apply, submit, hdec, htif, ↓reduceIte, out, looped] using hsum

end Nanobook
