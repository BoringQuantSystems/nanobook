/-
The tombstone queue matches the plain FIFO lists.
Each allowed step keeps `holds`.
-/

import Nanobook.Laws
import Nanobook.Proofs.WellFormed

namespace Nanobook

open List

def priceBefore (side : Side) (p q : Int) : Prop :=
  match side with
  | .buy => p > q
  | .sell => p < q

def leadingZeros : List Nat → Nat
  | [] => 0
  | x :: rest => if x == 0 then leadingZeros rest + 1 else 0

def dropLeadZeros : List Nat → List Nat
  | [] => []
  | x :: rest => if x == 0 then dropLeadZeros rest else x :: rest

def popSpan : List Nat → Nat
  | [] => 0
  | x :: rest => if x == 0 then popSpan rest + 1 else 1

def afterPop : List Nat → List Nat
  | [] => []
  | x :: rest => if x == 0 then afterPop rest else rest

def idxBefore : List Nat → Nat → Nat
  | [], _ => 0
  | h :: t, a => if h == a then 0 else idxBefore t a + 1

def qslot (side : Side) (price : Int) (qty pos : Nat) : QueueOrder where
  side := side
  price := price
  remaining := qty
  position_in_level := pos
  active := true

theorem comesFirst_true {side : Side} {price here : Int} :
    comesFirst side price here = true ↔ priceBefore side price here := by
  cases side <;> simp [comesFirst, priceBefore]

theorem priceBefore_of_not {side : Side} {price here : Int}
    (hnot : ¬ priceBefore side price here) (hne : price ≠ here) :
    priceBefore side here price := by
  cases side <;> simp [priceBefore] at hnot ⊢ <;> omega

theorem priceBefore_ne {side : Side} {p q : Int} (h : priceBefore side p q) : p ≠ q := by
  cases side <;> simp [priceBefore] at h <;> omega

theorem priceBefore_trans {side : Side} {a b c : Int}
    (h1 : priceBefore side a b) (h2 : priceBefore side b c) :
    priceBefore side a c := by
  cases side <;> simp [priceBefore] at h1 h2 ⊢ <;> omega

theorem liveIds_append_nz {ids : List Nat} {a : Nat} (ha : a ≠ 0) :
    liveIds (ids ++ [a]) = liveIds ids ++ [a] := by
  induction ids with
  | nil => simp [liveIds, ha]
  | cons head tail ih => cases head <;> simp [liveIds, ih]

theorem zeroCount_append_nz {ids : List Nat} {a : Nat} (ha : a ≠ 0) :
    zeroCount (ids ++ [a]) = zeroCount ids := by
  induction ids with
  | nil => cases a <;> simp_all [zeroCount]
  | cons head tail ih => cases head <;> simp [zeroCount, ih]

theorem liveIds_of_zero {ids : List Nat} (h : zeroCount ids = 0) : liveIds ids = ids := by
  induction ids with
  | nil => simp [liveIds]
  | cons head tail ih =>
    cases head with
    | zero => simp [zeroCount] at h
    | succ _ => simp [zeroCount] at h; simp [liveIds, ih h]

theorem liveIds_liveIds (ids : List Nat) : liveIds (liveIds ids) = liveIds ids := by
  induction ids with
  | nil => simp [liveIds]
  | cons head tail ih => cases head <;> simp [liveIds, ih]

theorem zeroCount_liveIds (ids : List Nat) : zeroCount (liveIds ids) = 0 := by
  induction ids with
  | nil => simp [liveIds, zeroCount]
  | cons head tail ih => cases head <;> simp [liveIds, zeroCount, ih]

theorem not_mem_live_zero (ids : List Nat) : 0 ∉ liveIds ids := by
  induction ids with
  | nil => simp [liveIds]
  | cons head tail ih => cases head <;> simp [liveIds, ih]

theorem mem_liveIds {ids : List Nat} {a : Nat} : a ∈ liveIds ids ↔ a ≠ 0 ∧ a ∈ ids := by
  induction ids with
  | nil => simp [liveIds]
  | cons head tail ih =>
    cases head with
    | zero =>
      simp [liveIds, ih]
      intro hne he
      exact absurd he hne
    | succ n =>
      simp [liveIds, ih]
      constructor
      · intro h
        rcases h with he | hr
        · subst he
          exact ⟨Nat.succ_ne_zero n, Or.inl rfl⟩
        · rcases hr with ⟨hne, ht⟩
          exact ⟨hne, Or.inr ht⟩
      · intro h
        rcases h with ⟨hne, hm⟩
        rcases hm with he | ht
        · exact Or.inl he
        · exact Or.inr ⟨hne, ht⟩

theorem getAt_lt {ids : List Nat} {i a : Nat} (h : getAt ids i = some a) : i < ids.length := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero => simp
    | succ j =>
      simp [getAt] at h
      exact Nat.succ_lt_succ (ih h)

theorem mem_of_getAt {ids : List Nat} {i a : Nat} (h : getAt ids i = some a) : a ∈ ids := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero =>
      simp [getAt] at h
      subst h
      exact mem_cons_self
    | succ j =>
      simp [getAt] at h
      exact mem_cons_of_mem _ (ih h)

theorem mem_live_of_getAt {ids : List Nat} {i a : Nat}
    (h : getAt ids i = some a) (ha : a ≠ 0) : a ∈ liveIds ids := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero =>
      simp [getAt] at h
      cases head with
      | zero => exact absurd h.symm ha
      | succ _ => simp [liveIds, h]
    | succ j =>
      simp [getAt] at h
      cases head with
      | zero =>
        simp [liveIds]
        exact ih h
      | succ _ =>
        simp [liveIds]
        exact Or.inr (ih h)

theorem getAt_append_lt {ids : List Nat} {a i : Nat} (h : i < ids.length) :
    getAt (ids ++ [a]) i = getAt ids i := by
  induction ids generalizing i with
  | nil => simp at h
  | cons head tail ih =>
    cases i with
    | zero => simp [getAt, cons_append]
    | succ j =>
      simp [getAt, cons_append]
      exact ih (Nat.lt_of_succ_lt_succ h)

theorem getAt_append_last (ids : List Nat) (a : Nat) :
    getAt (ids ++ [a]) ids.length = some a := by
  induction ids with
  | nil => simp [getAt]
  | cons head tail ih => simp [getAt, length_cons, cons_append, ih]

theorem getAt_setAt_self {ids : List Nat} {i a v : Nat} (h : getAt ids i = some a) :
    getAt (setAt ids i v) i = some v := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero => simp [setAt, getAt]
    | succ j =>
      simp [getAt] at h
      simp [setAt, getAt]
      exact ih h

theorem getAt_setAt_ne {ids : List Nat} {i j a v : Nat}
    (hne : i ≠ j) (h : getAt ids j = some a) :
    getAt (setAt ids i v) j = some a := by
  induction ids generalizing i j with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases j with
    | zero =>
      cases i with
      | zero => exact absurd rfl hne
      | succ _ => simp [setAt, getAt] at h ⊢; exact h
    | succ j' =>
      cases i with
      | zero => simp [setAt, getAt] at h ⊢; exact h
      | succ k =>
        simp [getAt] at h
        simp [setAt, getAt]
        exact ih (by simpa using hne) h

theorem liveIds_setAt_zero {ids : List Nat} {i oid : Nat}
    (h : getAt ids i = some oid) (hoid : oid ≠ 0) (hn : (liveIds ids).Nodup) :
    liveIds (setAt ids i 0) = removeId (liveIds ids) oid := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero =>
      simp [getAt] at h
      subst h
      simp [setAt, liveIds, removeId, hoid]
    | succ j =>
      simp [getAt] at h
      cases head with
      | zero =>
        simp [setAt, liveIds]
        exact ih h (by simpa [liveIds] using hn)
      | succ n =>
        have hn1 := nodup_cons.mp (by simpa [liveIds] using hn)
        have hmem : oid ∈ liveIds tail := mem_live_of_getAt h hoid
        have hne : n + 1 ≠ oid := by
          intro heq
          exact hn1.1 (by simpa [heq] using hmem)
        simp [setAt, liveIds, removeId, hne, ih h hn1.2]

theorem zeroCount_setAt_zero {ids : List Nat} {i oid : Nat}
    (h : getAt ids i = some oid) (hoid : oid ≠ 0) :
    zeroCount (setAt ids i 0) = zeroCount ids + 1 := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero =>
      simp [getAt] at h
      subst h
      simp [setAt, zeroCount, hoid]
    | succ j =>
      simp [getAt] at h
      cases head <;> simp [setAt, zeroCount, ih h]

theorem not_mem_live_setAt {ids : List Nat} {i oid : Nat}
    (h : getAt ids i = some oid) (hoid : oid ≠ 0) (hn : (liveIds ids).Nodup) :
    oid ∉ liveIds (setAt ids i 0) := by
  rw [liveIds_setAt_zero h hoid hn]
  exact removeId_not_mem hn oid

theorem findSlot_upsert (orders : List (Nat × QueueOrder)) (oid : Nat) (o : QueueOrder)
    (j : Nat) :
    findSlot (upsertSlot orders oid o) j =
      if j = oid then some o else findSlot orders j := by
  induction orders with
  | nil =>
    by_cases hj : j = oid
    · simp [findSlot, upsertSlot, hj]
    · simp [findSlot, upsertSlot, hj]
      exact Ne.symm hj
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid <;> by_cases hj : j = oid
    · simp [upsertSlot, findSlot, hk, hj]
    · simp [upsertSlot, findSlot, hk, Ne.symm hj]
      intro contra
      exact absurd contra hj
    · have hkj : k ≠ j := fun heq => hk (heq.trans hj)
      simp [upsertSlot, findSlot, hk, hj, hkj]
      rw [hj] at ih
      simpa using ih
    · simp [upsertSlot, findSlot, hk, hj, Ne.symm hj, ih]

theorem remainOf_upsert (orders : List (Nat × QueueOrder)) (oid : Nat) (o : QueueOrder)
    (j : Nat) :
    remainOf (upsertSlot orders oid o) j =
      if j = oid then o.remaining else remainOf orders j := by
  simp [remainOf, findSlot_upsert]
  by_cases hj : j = oid <;> simp [hj]

theorem findSlot_none_key {orders : List (Nat × QueueOrder)} {oid : Nat}
    (h : findSlot orders oid = none) : oid ∉ slotKeys orders := by
  induction orders with
  | nil => simp [slotKeys]
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid
    · simp [findSlot, hk] at h
    · simp [slotKeys, hk]
      simp [findSlot, hk] at h
      exact ⟨Ne.symm hk, ih h⟩

theorem slotKeys_upsert_fresh {orders : List (Nat × QueueOrder)} {oid : Nat} {o : QueueOrder}
    (h : findSlot orders oid = none) :
    slotKeys (upsertSlot orders oid o) = slotKeys orders ++ [oid] := by
  induction orders with
  | nil => simp [upsertSlot, slotKeys]
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid
    · simp [findSlot, hk] at h
    · simp [findSlot, hk] at h
      simp [upsertSlot, slotKeys, hk, ih h]

theorem nodup_snoc {ids : List Nat} {a : Nat} (hn : ids.Nodup) (ha : a ∉ ids) :
    (ids ++ [a]).Nodup := by
  induction ids with
  | nil => simp
  | cons head tail ih =>
    have hn1 := nodup_cons.mp hn
    have hta : a ∉ tail := fun ht => ha (mem_cons_of_mem _ ht)
    have hha : head ≠ a := fun heq => ha (heq ▸ mem_cons_self)
    refine nodup_cons.mpr ⟨?_, ih hn1.2 hta⟩
    intro hm
    rcases mem_append.mp hm with ht | hs
    · exact hn1.1 ht
    · simp at hs
      exact hha hs

theorem slotKeys_upsert_nodup {orders : List (Nat × QueueOrder)} {oid : Nat} {o : QueueOrder}
    (hn : (slotKeys orders).Nodup) (h : findSlot orders oid = none) :
    (slotKeys (upsertSlot orders oid o)).Nodup := by
  rw [slotKeys_upsert_fresh h]
  exact nodup_snoc hn (findSlot_none_key h)

theorem findSlot_setPos_self {orders : List (Nat × QueueOrder)} {oid pos : Nat} {s : QueueOrder}
    (h : findSlot orders oid = some s) :
    findSlot (setPos orders oid pos) oid = some { s with position_in_level := pos } := by
  induction orders with
  | nil => simp [findSlot] at h
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid
    · simp [findSlot, hk] at h
      simp [setPos, findSlot, hk, h]
    · simp [findSlot, hk] at h
      simp [setPos, findSlot, hk, ih h]

theorem findSlot_setPos_ne {orders : List (Nat × QueueOrder)} {oid other pos : Nat}
    (hne : oid ≠ other) :
    findSlot (setPos orders oid pos) other = findSlot orders other := by
  induction orders with
  | nil => simp [setPos, findSlot]
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid
    · simp [setPos, findSlot, hk, hne]
    · by_cases hk' : k = other
      · simp [setPos, findSlot, hk, hk', Ne.symm hne]
      · simp [setPos, findSlot, hk, hk', ih]

theorem findSlot_setPos_none {orders : List (Nat × QueueOrder)} {oid pos : Nat}
    (h : findSlot orders oid = none) :
    findSlot (setPos orders oid pos) oid = none := by
  induction orders with
  | nil => simp [setPos, findSlot]
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid
    · simp [findSlot, hk] at h
    · simp [findSlot, hk] at h
      simp [setPos, findSlot, hk, ih h]

theorem remainOf_setPos (orders : List (Nat × QueueOrder)) (oid pos j : Nat) :
    remainOf (setPos orders oid pos) j = remainOf orders j := by
  by_cases hj : j = oid
  · cases h : findSlot orders oid with
    | none => simp [remainOf, hj, h, findSlot_setPos_none h]
    | some s => simp [remainOf, hj, h, findSlot_setPos_self h]
  · simp [remainOf, findSlot_setPos_ne (Ne.symm hj)]

theorem slotKeys_setPos (orders : List (Nat × QueueOrder)) (oid pos : Nat) :
    slotKeys (setPos orders oid pos) = slotKeys orders := by
  induction orders with
  | nil => simp [setPos, slotKeys]
  | cons entry tail ih =>
    rcases entry with ⟨k, _⟩
    by_cases hk : k = oid <;> simp [setPos, slotKeys, hk, ih]

theorem liveSum_liveIds (ids : List Nat) (orders : List (Nat × QueueOrder)) :
    liveSum (liveIds ids) orders = liveSum ids orders := by
  induction ids with
  | nil => simp [liveIds, liveSum]
  | cons head tail ih => cases head <;> simp [liveIds, liveSum, ih]

theorem liveSum_append_nz {ids : List Nat} {a : Nat} (orders : List (Nat × QueueOrder))
    (ha : a ≠ 0) :
    liveSum (ids ++ [a]) orders = liveSum ids orders + remainOf orders a := by
  induction ids with
  | nil => cases a <;> simp_all [liveSum]
  | cons head tail ih =>
    cases head <;> simp [liveSum, cons_append, ih]
    omega

theorem liveSum_upsert_skip {ids : List Nat} {orders : List (Nat × QueueOrder)}
    {oid : Nat} {o : QueueOrder} (h : oid ∉ liveIds ids) :
    liveSum ids (upsertSlot orders oid o) = liveSum ids orders := by
  induction ids with
  | nil => simp [liveSum]
  | cons head tail ih =>
    cases head with
    | zero =>
      simp [liveSum]
      exact ih (by simpa [liveIds] using h)
    | succ n =>
      have hhead : n + 1 ≠ oid := by
        intro heq
        have hoid : oid ≠ 0 := by
          rw [← heq]
          exact Nat.succ_ne_zero n
        exact h (by simp [liveIds, hoid, heq])
      have htail : oid ∉ liveIds tail := by
        intro hm
        exact h (by simp [liveIds, (mem_liveIds.mp hm).1, hm])
      simp [liveSum, remainOf_upsert, hhead, ih htail]

theorem liveSum_setAt_add {ids : List Nat} {orders : List (Nat × QueueOrder)} {i oid : Nat}
    (h : getAt ids i = some oid) (hoid : oid ≠ 0) (hn : (liveIds ids).Nodup) :
    liveSum (setAt ids i 0) orders + remainOf orders oid = liveSum ids orders := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero =>
      simp [getAt] at h
      cases head with
      | zero => exact absurd h.symm hoid
      | succ _ =>
        simp [setAt, liveSum, h]
        omega
    | succ j =>
      simp [getAt] at h
      cases head with
      | zero =>
        simp [setAt, liveSum]
        exact ih h (by simpa [liveIds] using hn)
      | succ n =>
        have hn1 := nodup_cons.mp (by simpa [liveIds] using hn)
        have hmem : oid ∈ liveIds tail := mem_live_of_getAt h hoid
        have hne : n + 1 ≠ oid := by
          intro heq
          exact hn1.1 (by simpa [heq] using hmem)
        have hih := ih h hn1.2
        simp [setAt, liveSum, hne, hih]
        omega

theorem firstLive_ne_zero {ids : List Nat} {oid : Nat} (h : firstLive ids = some oid) :
    oid ≠ 0 := by
  induction ids with
  | nil => simp [firstLive] at h
  | cons head tail ih =>
    cases head with
    | zero => exact ih (by simpa [firstLive] using h)
    | succ n =>
      simp [firstLive] at h
      rw [← h]
      exact Nat.succ_ne_zero n

theorem liveIds_dropLead (ids : List Nat) : liveIds (dropLeadZeros ids) = liveIds ids := by
  induction ids with
  | nil => simp [dropLeadZeros, liveIds]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [dropLeadZeros, liveIds, ih]
    | false =>
      have hne : head ≠ 0 := by simpa using h0
      cases head with
      | zero => exact absurd rfl hne
      | succ _ => simp [dropLeadZeros, liveIds, h0]

theorem liveSum_dropLead (ids : List Nat) (orders : List (Nat × QueueOrder)) :
    liveSum (dropLeadZeros ids) orders = liveSum ids orders := by
  induction ids with
  | nil => simp [dropLeadZeros, liveSum]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [dropLeadZeros, liveSum, ih]
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ => simp [dropLeadZeros, liveSum, h0]

theorem zeroCount_dropLead (ids : List Nat) :
    zeroCount (dropLeadZeros ids) = zeroCount ids - leadingZeros ids := by
  induction ids with
  | nil => simp [dropLeadZeros, leadingZeros, zeroCount]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [dropLeadZeros, leadingZeros, zeroCount, ih]
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ => simp [dropLeadZeros, leadingZeros, zeroCount, h0]

theorem leading_le_zero (ids : List Nat) : leadingZeros ids ≤ zeroCount ids := by
  induction ids with
  | nil => simp [leadingZeros, zeroCount]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [leadingZeros, zeroCount]
      omega
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ => simp [leadingZeros, h0]

theorem frontLoop_spec (orders : List Nat) (price : Int) (total popped tomb : Nat) :
    let r := frontLoop orders price total popped tomb
    r.1 = {
      price := price
      orders := dropLeadZeros orders
      total_quantity := total
      tombstone_count := tomb - leadingZeros orders
      popped_from_front := popped + leadingZeros orders
    } ∧ r.2 = firstLive orders := by
  induction orders generalizing popped tomb with
  | nil => simp [frontLoop, dropLeadZeros, leadingZeros, firstLive]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp only [frontLoop, dropLeadZeros, leadingZeros, firstLive, h0]
      rcases ih (popped + 1) (tomb - 1) with ⟨hL, hR⟩
      refine ⟨?_, hR⟩
      have hsub : (tomb - 1) - leadingZeros tail = tomb - (leadingZeros tail + 1) := by omega
      have hadd : (popped + 1) + leadingZeros tail = popped + (leadingZeros tail + 1) := by omega
      simp [hL, hsub, hadd]
    | false =>
      have hne : head ≠ 0 := by simpa using h0
      cases head with
      | zero => exact absurd rfl hne
      | succ _ => simp [frontLoop, dropLeadZeros, leadingZeros, firstLive, h0]

theorem popLoop_spec (orders : List Nat) (price : Int) (popped tomb total quantity : Nat) :
    let r := popLoop orders price popped tomb total quantity
    r.1 = {
      price := price
      orders := afterPop orders
      total_quantity := match firstLive orders with
        | none => total
        | some _ => total - quantity
      tombstone_count := tomb - leadingZeros orders
      popped_from_front := popped + popSpan orders
    } ∧ r.2 = firstLive orders := by
  induction orders generalizing popped tomb with
  | nil => simp [popLoop, afterPop, leadingZeros, popSpan, firstLive]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp only [popLoop, afterPop, leadingZeros, popSpan, firstLive, h0]
      rcases ih (popped + 1) (tomb - 1) with ⟨hL, hR⟩
      refine ⟨?_, hR⟩
      have hsub : (tomb - 1) - leadingZeros tail = tomb - (leadingZeros tail + 1) := by omega
      have hadd : (popped + 1) + popSpan tail = popped + (popSpan tail + 1) := by omega
      simp [hL, hsub, hadd]
    | false =>
      have hne : head ≠ 0 := by simpa using h0
      cases head with
      | zero => exact absurd rfl hne
      | succ _ => simp [popLoop, afterPop, leadingZeros, popSpan, firstLive, h0]

theorem getAt_dropLead {ids : List Nat} {i a : Nat}
    (h : getAt ids i = some a) (ha : a ≠ 0) :
    leadingZeros ids ≤ i ∧ getAt (dropLeadZeros ids) (i - leadingZeros ids) = some a := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases i with
    | zero =>
      simp [getAt] at h
      cases h0 : head == 0 with
      | true =>
        have hz : head = 0 := by simpa using h0
        exact absurd (h.symm.trans hz) ha
      | false =>
        have hne : head ≠ 0 := by simpa using h0
        have hl : leadingZeros (head :: tail) = 0 := by simp [leadingZeros, hne]
        have hd : dropLeadZeros (head :: tail) = head :: tail := by simp [dropLeadZeros, hne]
        simp [← h, getAt, hl, hd]
    | succ j =>
      simp [getAt] at h
      cases h0 : head == 0 with
      | true =>
        have hz : head = 0 := by simpa using h0
        subst hz
        rcases ih h with ⟨hle, hget⟩
        have hl : leadingZeros (0 :: tail) = leadingZeros tail + 1 := rfl
        have hd : dropLeadZeros (0 :: tail) = dropLeadZeros tail := rfl
        have hidx : (j + 1) - (leadingZeros tail + 1) = j - leadingZeros tail := by omega
        refine ⟨by rw [hl]; exact Nat.succ_le_succ hle, ?_⟩
        rw [hd, hl, hidx]
        exact hget
      | false =>
        cases head with
        | zero => simp at h0
        | succ n =>
          have hl : leadingZeros ((n + 1) :: tail) = 0 := by simp [leadingZeros]
          have hd : dropLeadZeros ((n + 1) :: tail) = (n + 1) :: tail := by simp [dropLeadZeros]
          simp [getAt, h, hl, hd]

theorem popSpan_eq (ids : List Nat) :
    popSpan ids = leadingZeros ids + match firstLive ids with
      | none => 0
      | some _ => 1 := by
  induction ids with
  | nil => simp [popSpan, leadingZeros, firstLive]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [popSpan, leadingZeros, firstLive, ih]
      omega
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ => simp [popSpan, leadingZeros, firstLive, h0]

theorem live_index_after {ids : List Nat} {i a b : Nat}
    (hf : firstLive ids = some b) (h : getAt ids i = some a) (ha : a ≠ 0) (hne : a ≠ b) :
    leadingZeros ids < i := by
  induction ids generalizing i with
  | nil => simp [firstLive] at hf
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      cases i with
      | zero =>
        simp [getAt] at h
        exact absurd h.symm ha
      | succ j =>
        simp [getAt, leadingZeros, firstLive] at h hf ⊢
        exact ih hf h
    | false =>
      cases head with
      | zero => simp at h0
      | succ n =>
        simp [firstLive] at hf
        simp [leadingZeros, h0]
        cases i with
        | zero =>
          simp [getAt] at h
          exact absurd (h.symm.trans hf) hne
        | succ _ => exact Nat.succ_pos _

theorem getAt_afterPop {ids : List Nat} {i a : Nat}
    (h : getAt ids i = some a) (ha : a ≠ 0) (hpast : leadingZeros ids < i) :
    popSpan ids ≤ i ∧ getAt (afterPop ids) (i - popSpan ids) = some a := by
  induction ids generalizing i with
  | nil => simp [getAt] at h
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      cases i with
      | zero => simp [leadingZeros] at hpast
      | succ j =>
        simp [getAt, leadingZeros] at h hpast
        rcases ih h hpast with ⟨hle, hget⟩
        have hp : popSpan (0 :: tail) = popSpan tail + 1 := rfl
        have hafter : afterPop (0 :: tail) = afterPop tail := rfl
        have hidx : (j + 1) - (popSpan tail + 1) = j - popSpan tail := by omega
        refine ⟨by rw [hp]; exact Nat.succ_le_succ hle, ?_⟩
        rw [hp, hafter, hidx]
        exact hget
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ =>
        simp [leadingZeros, h0] at hpast
        cases i with
        | zero => simp at hpast
        | succ j =>
          simp [getAt, popSpan, afterPop, h0] at h ⊢
          simpa using h

theorem liveIds_afterPop {ids : List Nat} {oid : Nat} (hf : firstLive ids = some oid) :
    liveIds ids = oid :: liveIds (afterPop ids) := by
  induction ids with
  | nil => simp [firstLive] at hf
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [liveIds, afterPop, firstLive] at hf ⊢
      exact ih hf
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ =>
        simp [firstLive] at hf
        subst hf
        simp [liveIds, afterPop, h0]

theorem liveSum_afterPop {ids : List Nat} {oid : Nat} (orders : List (Nat × QueueOrder))
    (hf : firstLive ids = some oid) :
    liveSum ids orders = remainOf orders oid + liveSum (afterPop ids) orders := by
  have hl := liveIds_afterPop hf
  have hoid : oid ≠ 0 := firstLive_ne_zero hf
  have hsum :
      liveSum (liveIds ids) orders =
        remainOf orders oid + liveSum (liveIds (afterPop ids)) orders := by
    rw [hl]
    cases oid with
    | zero => exact absurd rfl hoid
    | succ _ => simp [liveSum]
  simpa [liveSum_liveIds] using hsum

theorem getAt_idxBefore {ids : List Nat} {a : Nat} (hm : a ∈ ids) (hn : ids.Nodup) :
    getAt ids (idxBefore ids a) = some a := by
  induction ids with
  | nil => simp at hm
  | cons head tail ih =>
    have hn1 := nodup_cons.mp hn
    cases h0 : head == a with
    | true =>
      have heqa : head = a := by simpa using h0
      simp [idxBefore, getAt, heqa]
    | false =>
      have hne : head ≠ a := by simpa using h0
      have ht : a ∈ tail := by
        rcases mem_cons.mp hm with heq | ht
        · exact absurd heq.symm hne
        · exact ht
      simp [idxBefore, getAt, h0, ih ht hn1.2]

theorem findSlot_rewrite_skip {level : QueueLevel} {ids : List Nat} {offset : Nat}
    {orders : List (Nat × QueueOrder)} {oid : Nat} (h : oid ∉ ids) :
    findSlot (rewriteLiveFrom level ids offset orders) oid = findSlot orders oid := by
  induction ids generalizing offset orders with
  | nil => simp [rewriteLiveFrom]
  | cons head tail ih =>
    have hne : head ≠ oid := by
      intro heq
      exact h (by simpa [heq] using mem_cons_self)
    have ht : oid ∉ tail := fun hm => h (mem_cons_of_mem _ hm)
    simp [rewriteLiveFrom, findSlot_setPos_ne hne, ih ht]

theorem findSlot_rewrite_mem {level : QueueLevel} {ids : List Nat} {offset : Nat}
    {orders : List (Nat × QueueOrder)} {oid : Nat} {s : QueueOrder}
    (hfind : findSlot orders oid = some s) (hm : oid ∈ ids) (hn : ids.Nodup) :
    findSlot (rewriteLiveFrom level ids offset orders) oid =
      some { s with position_in_level :=
        front_position level + offset + idxBefore ids oid } := by
  induction ids generalizing offset orders s with
  | nil => simp at hm
  | cons head tail ih =>
    have hn1 := nodup_cons.mp hn
    cases h0 : head == oid with
    | true =>
      have heq : head = oid := by simpa using h0
      have hskip : oid ∉ tail := by simpa [heq] using hn1.1
      have hrw := findSlot_rewrite_skip (level := level) (offset := offset + 1)
        (orders := setPos orders oid (front_position level + offset)) hskip
      have hpos := findSlot_setPos_self (pos := front_position level + offset) hfind
      have hidx : idxBefore (oid :: tail) oid = 0 := by simp [idxBefore]
      simp [rewriteLiveFrom, hrw, hpos, hidx, heq]
    | false =>
      have hne : head ≠ oid := by simpa using h0
      have ht : oid ∈ tail := by
        rcases mem_cons.mp hm with heq | ht
        · exact absurd heq.symm hne
        · exact ht
      have hfind' :
          findSlot (setPos orders head (front_position level + offset)) oid = some s := by
        simpa [findSlot_setPos_ne hne] using hfind
      have ih1 := ih (offset := offset + 1) hfind' ht hn1.2
      simp [rewriteLiveFrom, idxBefore, h0, ih1]
      omega

theorem front_fields (level : QueueLevel) :
    (front level).1 = {
      price := level.price
      orders := dropLeadZeros level.orders
      total_quantity := level.total_quantity
      tombstone_count := level.tombstone_count - leadingZeros level.orders
      popped_from_front := level.popped_from_front + leadingZeros level.orders
    } ∧ (front level).2 = firstLive level.orders := by
  simpa [front] using
    frontLoop_spec level.orders level.price level.total_quantity level.popped_from_front
      level.tombstone_count

theorem levelInv_front {level : QueueLevel} {orders : List (Nat × QueueOrder)} {side : Side}
    (h : levelInv level orders side) : levelInv (front level).1 orders side := by
  have hf := (front_fields level).1
  rcases h with ⟨hTomb, hTot, hEmpty, hLive⟩
  refine ⟨?_, ?_, ?_, ?_⟩
  · simpa [hf, zeroCount_dropLead, hTomb] using (leading_le_zero level.orders)
  · simp [hf, liveSum_dropLead, hTot]
  · simp [hf, is_empty, liveIds_dropLead, beq_iff_eq] at hEmpty ⊢
    exact hEmpty
  · intro oid hm
    have hm0 : oid ∈ liveIds level.orders := by simpa [hf, liveIds_dropLead] using hm
    have hnz : oid ≠ 0 := (mem_liveIds.mp hm0).1
    rcases hLive oid hm0 with ⟨s, hfind, hact, hrem, hside, hprice, hpop, hget⟩
    have hd := getAt_dropLead hget hnz
    refine ⟨s, hfind, hact, hrem, hside, ?_, ?_, ?_⟩
    · simpa [hf] using hprice
    · simp [hf]
      omega
    · simp [hf]
      have hidx :
          s.position_in_level - (level.popped_from_front + leadingZeros level.orders) =
            (s.position_in_level - level.popped_from_front) - leadingZeros level.orders := by
        omega
      simpa [hidx] using hd.2

theorem map_price_front (levels : List QueueLevel) (price : Int) :
    ((front_at levels price).1).map (·.price) = levels.map (·.price) := by
  induction levels with
  | nil => simp [front_at]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simp [front_at, hp, ih]
    · simp [front_at, hp, ih, (front_fields level).1]

theorem pairwise_front_at {levels : List QueueLevel} {price : Int} {R : Int → Int → Prop}
    (h : levels.Pairwise (fun a b => R a.price b.price)) :
    ((front_at levels price).1).Pairwise (fun a b => R a.price b.price) := by
  exact pairwise_map.mp (by rw [map_price_front]; exact pairwise_map.mpr h)

theorem levelInv_front_at {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} (h : ∀ level ∈ levels, levelInv level orders side) :
    ∀ level ∈ (front_at levels price).1, levelInv level orders side := by
  induction levels with
  | nil => simp [front_at]
  | cons level rest ih =>
    have hhead := h level mem_cons_self
    have htail : ∀ level ∈ rest, levelInv level orders side :=
      fun level' hm => h level' (mem_cons_of_mem _ hm)
    cases hp : (level.price == price)
    · intro level' hm
      simp [front_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using hhead
      · exact ih htail level' hm
    · intro level' hm
      simp [front_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using levelInv_front hhead
      · exact htail level' hm

theorem live_nonempty_front_at {levels : List QueueLevel} {price : Int}
    (h : ∀ level ∈ levels, liveIds level.orders ≠ []) :
    ∀ level ∈ (front_at levels price).1, liveIds level.orders ≠ [] := by
  induction levels with
  | nil => simp [front_at]
  | cons level rest ih =>
    have htail : ∀ level ∈ rest, liveIds level.orders ≠ [] :=
      fun level' hm => h level' (mem_cons_of_mem _ hm)
    cases hp : (level.price == price)
    · intro level' hm
      simp [front_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using h level mem_cons_self
      · exact ih htail level' hm
    · intro level' hm
      simp [front_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq, (front_fields level).1, liveIds_dropLead] using h level mem_cons_self
      · exact htail level' hm

theorem concatLive_front_at (levels : List QueueLevel) (price : Int) :
    concatLive (front_at levels price).1 = concatLive levels := by
  induction levels with
  | nil => simp [front_at, concatLive]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simp [front_at, concatLive, hp, ih]
    · simp [front_at, concatLive, hp, ih, (front_fields level).1, liveIds_dropLead]

theorem toPlainLevels_front (levels : List QueueLevel) (price : Int) :
    toPlainLevels (front_at levels price).1 = toPlainLevels levels := by
  induction levels with
  | nil => simp [front_at, toPlainLevels]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simp [front_at, toPlainLevels, hp, ih]
    · simp [front_at, toPlainLevels, toPlain, hp, ih, (front_fields level).1, liveIds_dropLead]

theorem live_ordersAt_front (levels : List QueueLevel) (price here : Int) :
    liveIds (ordersAt (front_at levels price).1 here) = liveIds (ordersAt levels here) := by
  induction levels with
  | nil => simp [front_at, ordersAt, liveIds]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · cases hh : (level.price == here)
      · simp [front_at, ordersAt, hp, hh, ih]
      · simp [front_at, ordersAt, hp, hh, ih]
    · have hprice : (front level).1.price = level.price := by
        simp [(front_fields level).1]
      cases hh : (level.price == here)
      · simp [front_at, ordersAt, hp, hh, hprice, ih]
      · simp [front_at, ordersAt, hp, hh, hprice, (front_fields level).1, liveIds_dropLead]

theorem holds_empty : holds QueueBook.empty Book.empty := by
  refine ⟨?_, ?_, ?_⟩
  · simp [abstractsBook, QueueBook.empty, Book.empty, toPlainLevels]
  · simp [bookInv, QueueBook.empty, concatLive, slotKeys, findSlot]
  · intro oid
    simp [QueueBook.empty, Book.empty, findSlot, findOrder]

theorem holds_front {q : QueueBook} {plain : Book} {side : Side} {price : Int}
    (h : holds q plain) :
    holds (applyQ (.front side price) q) (applyPlain (.front side price) plain) := by
  rcases h with ⟨hAbs, hBook, hLink⟩
  rcases hBook with ⟨hBid, hAsk, hBidI, hAskI, hBidN, hAskN, hDup, hKeys, hAct⟩
  rcases hAbs with ⟨hBids, hAsks⟩
  cases side with
  | buy =>
    refine ⟨?_, ?_, ?_⟩
    · simp [applyQ, applyPlain, setQueueLevels, queueLevels, abstractsBook, hAsks,
        toPlainLevels_front, hBids]
    · simp only [applyQ, applyPlain, setQueueLevels, queueLevels]
      refine ⟨pairwise_front_at hBid, hAsk, ?_, ?_, ?_, hAskN, ?_, hKeys, ?_⟩
      · exact levelInv_front_at hBidI
      · exact hAskI
      · exact live_nonempty_front_at hBidN
      · simpa [concatLive_front_at] using hDup
      · intro oid s hfind hactive
        have hmem := hAct oid s hfind hactive
        cases hs : s.side
        · simp [queueLevels, hs, live_ordersAt_front] at hmem ⊢
          exact hmem
        · simp [queueLevels, hs] at hmem ⊢
          exact hmem
    · intro oid
      have hOne := hLink oid
      simp [applyQ, applyPlain, setQueueLevels, linked] at hOne ⊢
      exact hOne
  | sell =>
    refine ⟨?_, ?_, ?_⟩
    · simp [applyQ, applyPlain, setQueueLevels, queueLevels, abstractsBook, hBids,
        toPlainLevels_front, hAsks]
    · simp only [applyQ, applyPlain, setQueueLevels, queueLevels]
      refine ⟨hBid, pairwise_front_at hAsk, ?_, ?_, hBidN, ?_, ?_, hKeys, ?_⟩
      · exact hBidI
      · exact levelInv_front_at hAskI
      · exact live_nonempty_front_at hAskN
      · simpa [concatLive_front_at] using hDup
      · intro oid s hfind hactive
        have hmem := hAct oid s hfind hactive
        cases hs : s.side
        · simp [queueLevels, hs] at hmem ⊢
          exact hmem
        · simp [queueLevels, hs, live_ordersAt_front] at hmem ⊢
          exact hmem
    · intro oid
      have hOne := hLink oid
      simp [applyQ, applyPlain, setQueueLevels, linked] at hOne ⊢
      exact hOne

theorem levelInv_new (price : Int) (orders : List (Nat × QueueOrder)) (side : Side) :
    levelInv (QueueLevel.new price) orders side := by
  simp [levelInv, QueueLevel.new, zeroCount, liveSum, liveIds, is_empty]

theorem levelInv_upsert_other {level : QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {oid : Nat} {o : QueueOrder}
    (h : levelInv level orders side) (hnot : oid ∉ liveIds level.orders) :
    levelInv level (upsertSlot orders oid o) side := by
  rcases h with ⟨hTomb, hTot, hEmpty, hLive⟩
  refine ⟨hTomb, ?_, hEmpty, ?_⟩
  · simpa [liveSum_upsert_skip hnot] using hTot
  · intro a ha
    have hne : a ≠ oid := by
      intro heq
      exact hnot (heq ▸ ha)
    rcases hLive a ha with ⟨s, hfind, hact, hrem, hside, hprice, hpop, hget⟩
    refine ⟨s, ?_, hact, hrem, hside, hprice, hpop, hget⟩
    simpa [findSlot_upsert, hne] using hfind

theorem levelInv_push {level : QueueLevel} {orders : List (Nat × QueueOrder)} {side : Side}
    {oid qty : Nat} (h : levelInv level orders side) (hid : oid ≠ 0) (hqty : 0 < qty)
    (hnot : oid ∉ liveIds level.orders) :
    levelInv (push_back level oid qty)
      (upsertSlot orders oid (qslot side level.price qty (next_position level))) side := by
  rcases h with ⟨hTomb, hTot, hEmpty, hLive⟩
  refine ⟨?_, ?_, ?_, ?_⟩
  · simp [push_back, zeroCount_append_nz hid, hTomb]
  · have hskip := liveSum_upsert_skip (orders := orders)
      (o := qslot side level.price qty (next_position level)) hnot
    have happ := liveSum_append_nz (ids := level.orders)
      (orders := upsertSlot orders oid (qslot side level.price qty (next_position level))) hid
    have hrem :
        remainOf (upsertSlot orders oid (qslot side level.price qty (next_position level))) oid =
          qty := by
      simp [remainOf_upsert, qslot]
    simp only [push_back]
    rw [happ, hskip, hTot, hrem]
  · simp [push_back, is_empty, liveIds_append_nz hid]
    intro _
    exact Nat.ne_of_gt hqty
  · intro a ha
    have hlive : liveIds (level.orders ++ [oid]) = liveIds level.orders ++ [oid] :=
      liveIds_append_nz hid
    simp [push_back] at ha
    rw [hlive] at ha
    rcases mem_append.mp ha with hold | hnew
    · have hne : a ≠ oid := by
        intro heq
        exact hnot (heq ▸ hold)
      rcases hLive a hold with ⟨s, hfind, hact, hrem, hside, hprice, hpop, hget⟩
      have hlt := getAt_lt hget
      refine ⟨s, ?_, hact, hrem, hside, ?_, hpop, ?_⟩
      · simpa [findSlot_upsert, hne] using hfind
      · simpa [push_back] using hprice
      · simpa [push_back, getAt_append_lt hlt] using hget
    · simp at hnew
      subst hnew
      refine ⟨qslot side level.price qty (next_position level), ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
      · simp [findSlot_upsert]
      · simp [qslot]
      · simp [qslot]
        exact hqty
      · simp [qslot]
      · simp [qslot, push_back]
      · simp [qslot, push_back, next_position]
      · have hidx :
            (level.popped_from_front + level.orders.length) - level.popped_from_front =
              level.orders.length := by omega
        simp [push_back, qslot, next_position, hidx, getAt_append_last]

theorem not_concat_of_fresh {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {oid : Nat}
    (hI : ∀ level ∈ levels, levelInv level orders side) (h : findSlot orders oid = none) :
    oid ∉ concatLive levels := by
  induction levels with
  | nil => simp [concatLive]
  | cons level rest ih =>
    intro hm
    simp [concatLive] at hm
    rcases hm with hl | hr
    · rcases (hI level mem_cons_self).2.2.2 oid hl with ⟨s, hfind, _⟩
      simp [hfind] at h
    · exact ih (fun level' hm => hI level' (mem_cons_of_mem _ hm)) hr

theorem before_all {side : Side} {level : QueueLevel} {rest : List QueueLevel} {price : Int}
    (hs : (level :: rest).Pairwise (fun a b => priceBefore side a.price b.price))
    (h : priceBefore side price level.price) :
    ∀ l ∈ level :: rest, priceBefore side price l.price := by
  intro l hm
  rcases mem_cons.mp hm with heq | ht
  · simpa [heq] using h
  · exact priceBefore_trans h ((pairwise_cons.mp hs).1 l ht)

theorem ordersAt_absent {levels : List QueueLevel} {price : Int}
    (h : ∀ level ∈ levels, level.price ≠ price) : ordersAt levels price = [] := by
  induction levels with
  | nil => simp [ordersAt]
  | cons level rest ih =>
    have hne : level.price ≠ price := h level mem_cons_self
    simp [ordersAt, hne, ih (fun l hm => h l (mem_cons_of_mem _ hm))]

theorem mem_price_insert {side : Side} {levels : List QueueLevel} {price : Int} {oid qty : Nat}
    {level' : QueueLevel} (hm : level' ∈ (insert_order side levels price oid qty).1) :
    level'.price = price ∨ ∃ old ∈ levels, level'.price = old.price := by
  induction levels generalizing level' with
  | nil =>
    simp [insert_order, push_back, QueueLevel.new] at hm
    exact Or.inl (by simpa [hm, push_back, QueueLevel.new])
  | cons level rest ih =>
    cases hcf : comesFirst side price level.price
    · cases hp : (price == level.price)
      · simp [insert_order, hcf, hp] at hm
        rcases hm with heq | hm
        · exact Or.inr ⟨level, mem_cons_self, by simpa [heq, push_back]⟩
        · rcases ih hm with hnew | ⟨old, hold, hpr⟩
          · exact Or.inl hnew
          · exact Or.inr ⟨old, mem_cons_of_mem _ hold, hpr⟩
      · simp [insert_order, hcf, hp] at hm
        have hprice : price = level.price := by simpa using hp
        rcases hm with heq | hm
        · exact Or.inl (by simp [heq, push_back, hprice])
        · exact Or.inr ⟨level', mem_cons_of_mem _ hm, rfl⟩
    · simp [insert_order, hcf] at hm
      rcases hm with heq | heq | hm
      · exact Or.inl (by simp [heq, push_back, QueueLevel.new])
      · exact Or.inr ⟨level, mem_cons_self, by simp [heq]⟩
      · exact Or.inr ⟨level', mem_cons_of_mem _ hm, rfl⟩

theorem pairwise_insert {side : Side} {levels : List QueueLevel} {price : Int} {oid qty : Nat}
    (hs : levels.Pairwise (fun a b => priceBefore side a.price b.price)) :
    (insert_order side levels price oid qty).1.Pairwise
      (fun a b => priceBefore side a.price b.price) := by
  induction levels with
  | nil => simp [insert_order]
  | cons level rest ih =>
    cases hcf : comesFirst side price level.price
    · cases hp : (price == level.price)
      · have hpair := pairwise_cons.mp hs
        have hnot : ¬ priceBefore side price level.price := by
          intro hb
          simp [comesFirst_true.mpr hb] at hcf
        have hne : price ≠ level.price := by simpa using hp
        have hback := priceBefore_of_not hnot hne
        have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              level :: (insert_order side rest price oid qty).1 := by
          simp [insert_order, hcf, hp]
        rw [hshape]
        refine pairwise_cons.mpr ⟨?_, ih hpair.2⟩
        intro level' hm
        rcases mem_price_insert hm with hnew | ⟨old, hold, hpr⟩
        · simpa [hnew] using hback
        · simpa [hpr] using hpair.1 old hold
      · have hpair := pairwise_cons.mp hs
        have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              push_back level oid qty :: rest := by
          simp [insert_order, hcf, hp]
        rw [hshape]
        refine pairwise_cons.mpr ⟨?_, hpair.2⟩
        intro level' hm
        simpa [push_back] using hpair.1 level' hm
    · have hbefore : priceBefore side price level.price := comesFirst_true.mp hcf
      have hshape :
          (insert_order side (level :: rest) price oid qty).1 =
            push_back (QueueLevel.new price) oid qty :: level :: rest := by
        simp [insert_order, hcf]
      rw [hshape]
      refine pairwise_cons.mpr ⟨?_, hs⟩
      intro level' hm
      rcases mem_cons.mp hm with heq | ht
      · simpa [heq, push_back, QueueLevel.new] using hbefore
      · exact priceBefore_trans hbefore ((pairwise_cons.mp hs).1 level' ht)

theorem ordersAt_insert_here {side : Side} {levels : List QueueLevel} {price : Int}
    {oid qty : Nat} (hs : levels.Pairwise (fun a b => priceBefore side a.price b.price))
    (hid : oid ≠ 0) :
    liveIds (ordersAt (insert_order side levels price oid qty).1 price) =
      liveIds (ordersAt levels price) ++ [oid] := by
  induction levels with
  | nil =>
    simp [insert_order, ordersAt, push_back, QueueLevel.new, liveIds, hid]
  | cons level rest ih =>
    cases hcf : comesFirst side price level.price
    · cases hp : (price == level.price)
      · have hne : level.price ≠ price := by
          intro heq
          simp [heq] at hp
        simp [insert_order, ordersAt, hcf, hp, hne, ih (pairwise_cons.mp hs).2]
      · have hprice : price = level.price := by simpa using hp
        have hno : comesFirst side level.price level.price = false := by
          cases side <;> simp [comesFirst]
        simp [insert_order, ordersAt, hcf, hp, hprice, hno, push_back, liveIds_append_nz hid]
    · have hbefore : priceBefore side price level.price := comesFirst_true.mp hcf
      have habsent : ordersAt (level :: rest) price = [] :=
        ordersAt_absent (fun l hm => Ne.symm (priceBefore_ne (before_all hs hbefore l hm)))
      simp [insert_order, ordersAt, hcf, push_back, QueueLevel.new, liveIds, hid, habsent]

theorem ordersAt_insert_other {side : Side} {levels : List QueueLevel} {price here : Int}
    {oid qty : Nat} (hp : here ≠ price) :
    ordersAt (insert_order side levels price oid qty).1 here = ordersAt levels here := by
  induction levels with
  | nil =>
    simp [insert_order, ordersAt, push_back, QueueLevel.new]
    exact Ne.symm hp
  | cons level rest ih =>
    cases hcf : comesFirst side price level.price
    · cases he : (price == level.price)
      · simp [insert_order, ordersAt, hcf, he, ih]
      · have hprice : price = level.price := by simpa using he
        have hno : comesFirst side level.price level.price = false := by
          cases side <;> simp [comesFirst]
        have hskip : level.price ≠ here := by
          intro heq
          exact hp (heq.symm.trans hprice.symm)
        simp [insert_order, ordersAt, hcf, he, hprice, hno, hskip, push_back]
    · have hfresh : price ≠ here := Ne.symm hp
      simp [insert_order, ordersAt, hcf, push_back, QueueLevel.new, hfresh]

theorem mem_concatLive {levels : List QueueLevel} {level : QueueLevel} {a : Nat}
    (hm : level ∈ levels) (ha : a ∈ liveIds level.orders) : a ∈ concatLive levels := by
  induction levels with
  | nil => cases hm
  | cons head rest ih =>
    rcases mem_cons.mp hm with heq | ht
    · have : a ∈ liveIds head.orders ++ concatLive rest :=
        mem_append.mpr (Or.inl (by simpa [heq] using ha))
      simpa [concatLive] using this
    · have : a ∈ liveIds head.orders ++ concatLive rest :=
        mem_append.mpr (Or.inr (ih ht))
      simpa [concatLive] using this

theorem levelInv_insert {side : Side} {levels : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {price : Int} {oid qty : Nat}
    (hI : ∀ level ∈ levels, levelInv level orders side)
    (hs : levels.Pairwise (fun a b => priceBefore side a.price b.price))
    (hid : oid ≠ 0) (hqty : 0 < qty) (hfresh : findSlot orders oid = none)
    (hnot : oid ∉ concatLive levels) :
    ∀ level ∈ (insert_order side levels price oid qty).1,
      levelInv level (upsertSlot orders oid
        (qslot side price qty (insert_order side levels price oid qty).2)) side := by
  induction levels with
  | nil =>
    intro level hm
    simp [insert_order] at hm
    simpa [hm, insert_order, next_position, QueueLevel.new] using
      levelInv_push (levelInv_new price orders side) hid hqty
        (by simp [QueueLevel.new, liveIds])
  | cons level rest ih =>
    have hhead := hI level mem_cons_self
    have htail : ∀ level ∈ rest, levelInv level orders side :=
      fun level' hm => hI level' (mem_cons_of_mem _ hm)
    have hnotHead : oid ∉ liveIds level.orders := by
      intro hm
      exact hnot (by simp [concatLive, hm])
    have hnotTail : oid ∉ concatLive rest := by
      intro hm
      exact hnot (by simp [concatLive, hm])
    cases hcf : comesFirst side price level.price
    · cases hp : (price == level.price)
      · have hshape :
            insert_order side (level :: rest) price oid qty =
              (level :: (insert_order side rest price oid qty).1,
                (insert_order side rest price oid qty).2) := by
          simp [insert_order, hcf, hp]
        intro level' hm
        rw [hshape] at hm
        rcases mem_cons.mp hm with heq | hm
        · simpa [heq, hshape] using levelInv_upsert_other hhead hnotHead
        · simpa [hshape] using ih htail (pairwise_cons.mp hs).2 hnotTail level' hm
      · have hprice : price = level.price := by simpa using hp
        have hno : comesFirst side level.price level.price = false := by
          cases side <;> simp [comesFirst]
        have hshape :
            insert_order side (level :: rest) price oid qty =
              (push_back level oid qty :: rest, next_position level) := by
          simp [insert_order, hcf, hp, hprice, hno]
        intro level' hm
        rw [hshape] at hm
        rcases mem_cons.mp hm with heq | hm
        · rw [heq]
          simpa [hshape] using (hprice.symm ▸ levelInv_push hhead hid hqty hnotHead)
        · have hmiss : oid ∉ liveIds level'.orders := by
            intro hmem
            exact hnotTail (mem_concatLive hm hmem)
          simpa [hshape] using levelInv_upsert_other (htail level' hm) hmiss
    · have hshape :
          insert_order side (level :: rest) price oid qty =
            (push_back (QueueLevel.new price) oid qty :: level :: rest,
              next_position (QueueLevel.new price)) := by
        simp [insert_order, hcf]
      intro level' hm
      rw [hshape] at hm
      rcases mem_cons.mp hm with heq | hm
      · simpa [heq, hshape, next_position, QueueLevel.new] using
          levelInv_push (levelInv_new price orders side) hid hqty
            (by simp [QueueLevel.new, liveIds])
      · have hmiss : oid ∉ liveIds level'.orders := by
          intro hmem
          exact hnot (mem_concatLive hm hmem)
        simpa [hshape] using levelInv_upsert_other (hI level' hm) hmiss

theorem toPlainLevels_insert {side : Side} {levels : List QueueLevel}
    {price : Int} {oid qty : Nat} (hid : oid ≠ 0) :
    toPlainLevels (insert_order side levels price oid qty).1 =
      match side with
      | .buy => insertBid (toPlainLevels levels) price oid
      | .sell => insertAsk (toPlainLevels levels) price oid := by
  induction levels with
  | nil =>
    cases side <;> simp [insert_order, toPlainLevels, toPlain, push_back, QueueLevel.new,
      insertBid, insertAsk, liveIds, hid]
  | cons level rest ih =>
    cases side with
    | buy =>
      cases hcf : comesFirst .buy price level.price
      · cases hp : (price == level.price)
        · have hngt : ¬ price > level.price := by
            intro hgt
            simp [comesFirst, hgt] at hcf
          have hneq : price ≠ level.price := by simpa using hp
          simp [insert_order, toPlainLevels, toPlain, insertBid, hcf, hp, hngt, hneq, ih]
        · have hprice : price = level.price := by simpa using hp
          have hno : comesFirst .buy level.price level.price = false := by
            simp [comesFirst]
          have hngt : ¬ price > level.price := by
            rw [hprice]
            intro hgt
            omega
          simp [insert_order, toPlainLevels, toPlain, insertBid, push_back, hcf, hp, hprice,
            hno, hngt, liveIds_append_nz hid]
      · have hgt : price > level.price := by
          simpa [priceBefore] using comesFirst_true.mp hcf
        simp [insert_order, toPlainLevels, toPlain, insertBid, push_back, QueueLevel.new,
          liveIds, hid, hcf, hgt]
    | sell =>
      cases hcf : comesFirst .sell price level.price
      · cases hp : (price == level.price)
        · have hnlt : ¬ price < level.price := by
            intro hlt
            simp [comesFirst, hlt] at hcf
          have hneq : price ≠ level.price := by simpa using hp
          simp [insert_order, toPlainLevels, toPlain, insertAsk, hcf, hp, hnlt, hneq, ih]
        · have hprice : price = level.price := by simpa using hp
          have hno : comesFirst .sell level.price level.price = false := by
            simp [comesFirst]
          have hnlt : ¬ price < level.price := by
            rw [hprice]
            intro hlt
            omega
          simp [insert_order, toPlainLevels, toPlain, insertAsk, push_back, hcf, hp, hprice,
            hno, hnlt, liveIds_append_nz hid]
      · have hlt : price < level.price := by
          simpa [priceBefore] using comesFirst_true.mp hcf
        simp [insert_order, toPlainLevels, toPlain, insertAsk, push_back, QueueLevel.new,
          liveIds, hid, hcf, hlt]

theorem live_nonempty_insert {side : Side} {levels : List QueueLevel}
    {price : Int} {oid qty : Nat}
    (hid : oid ≠ 0) (hN : ∀ level ∈ levels, liveIds level.orders ≠ []) :
    ∀ level ∈ (insert_order side levels price oid qty).1, liveIds level.orders ≠ [] := by
  induction levels with
  | nil =>
    intro level hm
    simp [insert_order] at hm
    simp [hm, push_back, QueueLevel.new, liveIds, hid]
  | cons level rest ih =>
    have hhead := hN level mem_cons_self
    have htail : ∀ level ∈ rest, liveIds level.orders ≠ [] :=
      fun level' hm => hN level' (mem_cons_of_mem _ hm)
    cases hcf : comesFirst side price level.price
    · cases hp : (price == level.price)
      · have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              level :: (insert_order side rest price oid qty).1 := by
          simp [insert_order, hcf, hp]
        intro level' hm
        rw [hshape] at hm
        rcases mem_cons.mp hm with heq | hm
        · simpa [heq] using hhead
        · exact ih htail level' hm
      · have hprice : price = level.price := by simpa using hp
        have hno : comesFirst side level.price level.price = false := by
          cases side <;> simp [comesFirst]
        have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              push_back level oid qty :: rest := by
          simp [insert_order, hcf, hp, hprice, hno]
        intro level' hm
        rw [hshape] at hm
        rcases mem_cons.mp hm with heq | hm
        · rw [heq]
          intro hempty
          have hmem : oid ∈ liveIds (level.orders ++ [oid]) := by
            rw [liveIds_append_nz hid]
            exact mem_append_right _ (by simp)
          simp [push_back] at hempty
          simp [hempty] at hmem
        · exact htail level' hm
    · have hshape :
          (insert_order side (level :: rest) price oid qty).1 =
            push_back (QueueLevel.new price) oid qty :: level :: rest := by
        simp [insert_order, hcf]
      intro level' hm
      rw [hshape] at hm
      rcases mem_cons.mp hm with heq | hm
      · rw [heq]
        intro hempty
        simp [push_back, QueueLevel.new, liveIds, hid] at hempty
      · exact hN level' hm

theorem mem_concat_insert {side : Side} {levels : List QueueLevel}
    {price : Int} {oid qty a : Nat} (hid : oid ≠ 0) :
    a ∈ concatLive (insert_order side levels price oid qty).1 ↔
      a = oid ∨ a ∈ concatLive levels := by
  induction levels with
  | nil =>
    simp [insert_order, concatLive, push_back, QueueLevel.new, liveIds, hid]
  | cons level rest ih =>
    cases hcf : comesFirst side price level.price
    · cases hp : (price == level.price)
      · have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              level :: (insert_order side rest price oid qty).1 := by
          simp [insert_order, hcf, hp]
        rw [hshape]
        simp only [concatLive]
        constructor
        · intro hm
          rcases mem_append.mp hm with hl | hr
          · exact Or.inr (mem_append.mpr (Or.inl hl))
          · rcases ih.mp hr with hr | hr
            · exact Or.inl hr
            · exact Or.inr (mem_append.mpr (Or.inr hr))
        · intro hm
          rcases hm with hm | hm
          · exact mem_append.mpr (Or.inr (ih.mpr (Or.inl hm)))
          · rcases mem_append.mp hm with hl | hr
            · exact mem_append.mpr (Or.inl hl)
            · exact mem_append.mpr (Or.inr (ih.mpr (Or.inr hr)))
      · have hprice : price = level.price := by simpa using hp
        have hno : comesFirst side level.price level.price = false := by
          cases side <;> simp [comesFirst]
        have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              push_back level oid qty :: rest := by
          simp [insert_order, hcf, hp, hprice, hno]
        rw [hshape]
        simp only [concatLive, push_back, liveIds_append_nz hid]
        constructor
        · intro hm
          rcases mem_append.mp hm with hl | hr
          · rcases mem_append.mp hl with hl | hl
            · exact Or.inr (mem_append.mpr (Or.inl hl))
            · simp at hl
              exact Or.inl hl
          · exact Or.inr (mem_append.mpr (Or.inr hr))
        · intro hm
          rcases hm with hm | hm
          · exact mem_append.mpr (Or.inl (mem_append.mpr (Or.inr (by simp [hm]))))
          · rcases mem_append.mp hm with hl | hr
            · exact mem_append.mpr (Or.inl (mem_append.mpr (Or.inl hl)))
            · exact mem_append.mpr (Or.inr hr)
    · have hshape :
          (insert_order side (level :: rest) price oid qty).1 =
            push_back (QueueLevel.new price) oid qty :: level :: rest := by
        simp [insert_order, hcf]
      rw [hshape]
      simp only [concatLive, push_back, QueueLevel.new, liveIds, hid]
      constructor
      · intro hm
        rcases mem_append.mp hm with hl | hr
        · have hl' : a = oid := by simpa [liveIds, hid] using hl
          exact Or.inl hl'
        · exact Or.inr hr
      · intro hm
        rcases hm with hm | hm
        · exact mem_append.mpr (Or.inl (by simp [liveIds, hid, hm]))
        · exact mem_append.mpr (Or.inr hm)

theorem nodup_of_append_right {l r : List Nat} (hn : (l ++ r).Nodup) : r.Nodup := by
  induction l with
  | nil =>
    rw [List.nil_append] at hn
    exact hn
  | cons head tail ih =>
    rw [cons_append] at hn
    exact ih (nodup_cons.mp hn).2

theorem nodup_mid {l r : List Nat} {a : Nat}
    (hn : (l ++ r).Nodup) (ha : a ∉ l ++ r) : (l ++ a :: r).Nodup := by
  induction l with
  | nil =>
    rw [List.nil_append] at hn ha ⊢
    exact nodup_cons.mpr ⟨ha, hn⟩
  | cons head tail ih =>
    rw [cons_append] at hn ha
    have hn1 := nodup_cons.mp hn
    have hah : a ≠ head := by
      intro heq
      exact ha (mem_cons.mpr (Or.inl heq))
    have hat : a ∉ tail ++ r := by
      intro hm
      exact ha (mem_cons.mpr (Or.inr hm))
    have hih := ih hn1.2 hat
    rw [cons_append]
    refine nodup_cons.mpr ⟨?_, hih⟩
    intro hm
    rcases mem_append.mp hm with ht | hs
    · exact hn1.1 (mem_append.mpr (Or.inl ht))
    · rcases mem_cons.mp hs with heq | hr
      · exact hah heq.symm
      · exact hn1.1 (mem_append.mpr (Or.inr hr))

theorem nodup_append_swap {l r : List Nat} (hn : (l ++ r).Nodup) : (r ++ l).Nodup := by
  induction l with
  | nil =>
    rw [List.nil_append] at hn
    rw [List.append_nil]
    exact hn
  | cons head tail ih =>
    rw [cons_append] at hn
    have hn1 := nodup_cons.mp hn
    exact nodup_mid (ih hn1.2) (by
      intro hm
      rcases mem_append.mp hm with hr | ht
      · exact hn1.1 (mem_append.mpr (Or.inr hr))
      · exact hn1.1 (mem_append.mpr (Or.inl ht)))

theorem nodup_left_of_sub {l r r' : List Nat} {a : Nat}
    (hn : (l ++ r).Nodup) (hr : r'.Nodup) (ha : a ∉ l ++ r)
    (hsub : ∀ x ∈ r', x = a ∨ x ∈ r) : (l ++ r').Nodup := by
  induction l with
  | nil =>
    rw [List.nil_append]
    exact hr
  | cons head tail ih =>
    rw [cons_append] at hn ha
    have hn1 := nodup_cons.mp hn
    have hah : head ≠ a := by
      intro heq
      exact ha (mem_cons.mpr (Or.inl heq.symm))
    have htail : a ∉ tail ++ r := by
      intro hm
      exact ha (mem_cons.mpr (Or.inr hm))
    have hih := ih hn1.2 htail
    rw [cons_append]
    refine nodup_cons.mpr ⟨?_, hih⟩
    intro hm
    rcases mem_append.mp hm with ht | hs
    · exact hn1.1 (mem_append.mpr (Or.inl ht))
    · rcases hsub head hs with heq | hin
      · exact hah heq
      · exact hn1.1 (mem_append.mpr (Or.inr hin))

theorem nodup_concat_insert {side : Side} {levels : List QueueLevel}
    {price : Int} {oid qty : Nat} {other : List Nat}
    (hid : oid ≠ 0) (hnot : oid ∉ concatLive levels ++ other)
    (hn : (concatLive levels ++ other).Nodup) :
    (concatLive (insert_order side levels price oid qty).1 ++ other).Nodup := by
  induction levels with
  | nil =>
    simp only [insert_order, concatLive, push_back, QueueLevel.new, liveIds, hid,
      List.nil_append, List.singleton_append] at hnot hn ⊢
    exact nodup_cons.mpr ⟨hnot, hn⟩
  | cons level rest ih =>
    cases hcf : comesFirst side price level.price
    · cases hp : (price == level.price)
      · have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              level :: (insert_order side rest price oid qty).1 := by
          simp [insert_order, hcf, hp]
        rw [hshape]
        simp only [concatLive, append_assoc]
        have hnTail : (concatLive rest ++ other).Nodup :=
          nodup_of_append_right (by simpa [concatLive, append_assoc] using hn)
        have hnotTail : oid ∉ concatLive rest ++ other := by
          intro hm
          exact hnot (by
            simp only [concatLive]
            rw [append_assoc]
            exact mem_append.mpr (Or.inr hm))
        have hih := ih hnotTail hnTail
        have hsub : ∀ x ∈ concatLive (insert_order side rest price oid qty).1 ++ other,
            x = oid ∨ x ∈ concatLive rest ++ other := by
          intro x hx
          rcases mem_append.mp hx with hx | hx
          · rcases (mem_concat_insert hid).mp hx with hx | hx
            · exact Or.inl hx
            · exact Or.inr (mem_append.mpr (Or.inl hx))
          · exact Or.inr (mem_append.mpr (Or.inr hx))
        exact nodup_left_of_sub (r' := concatLive (insert_order side rest price oid qty).1 ++ other)
          (by simpa [concatLive, append_assoc] using hn) hih
          (by
            intro hm
            exact hnot (by simpa [concatLive, append_assoc] using hm))
          hsub
      · have hprice : price = level.price := by simpa using hp
        have hno : comesFirst side level.price level.price = false := by
          cases side <;> simp [comesFirst]
        have hshape :
            (insert_order side (level :: rest) price oid qty).1 =
              push_back level oid qty :: rest := by
          simp [insert_order, hcf, hp, hprice, hno]
        rw [hshape]
        simp only [concatLive, push_back, liveIds_append_nz hid, append_assoc]
        exact nodup_mid (by simpa [concatLive, append_assoc] using hn)
          (by
            intro hm
            exact hnot (by simpa [concatLive, append_assoc] using hm))
    · have hshape :
          (insert_order side (level :: rest) price oid qty).1 =
            push_back (QueueLevel.new price) oid qty :: level :: rest := by
        simp [insert_order, hcf]
      rw [hshape]
      simp only [concatLive, push_back, QueueLevel.new, List.nil_append, liveIds, hid,
        List.singleton_append, cons_append]
      exact nodup_cons.mpr ⟨by simpa [concatLive] using hnot, by simpa [concatLive] using hn⟩

theorem oid_mem_insert {side : Side} {levels : List QueueLevel} {price : Int}
    {oid qty : Nat}
    (hs : levels.Pairwise (fun a b => priceBefore side a.price b.price))
    (hid : oid ≠ 0) :
    oid ∈ liveIds (ordersAt (insert_order side levels price oid qty).1 price) := by
  rw [ordersAt_insert_here hs hid]
  exact mem_append_right _ (by simp)

theorem mem_ordersAt_insert_old {side : Side} {levels : List QueueLevel}
    {price here : Int} {oid qty j : Nat}
    (hs : levels.Pairwise (fun a b => priceBefore side a.price b.price))
    (hid : oid ≠ 0) (hj : j ∈ liveIds (ordersAt levels here)) :
    j ∈ liveIds (ordersAt (insert_order side levels price oid qty).1 here) := by
  by_cases hhere : here = price
  · subst hhere
    rw [ordersAt_insert_here hs hid]
    exact mem_append.mpr (Or.inl hj)
  · rw [ordersAt_insert_other hhere]
    exact hj

theorem linked_upsert {q : QueueBook} {plain : Book} {oid : Nat} {s : QueueOrder}
    {o : Order} (hLink : linked q plain) (hfresh : findSlot q.orders oid = none)
    (hid : o.id = oid) (hagree : slotAgrees s o) :
    linked { q with orders := upsertSlot q.orders oid s } (writeOrder plain o) := by
  intro j
  have hOne := hLink j
  simp [writeOrder, findSlot_upsert, findOrder_upsert, hid] at hOne ⊢
  by_cases hj : j = oid
  · rw [hj] at hOne ⊢
    cases hfo : findOrder plain.orders oid with
    | none =>
      simp [hfresh, hfo, hagree]
      exact hid
    | some _ =>
      simp [hfresh, hfo] at hOne
  · simp [hj] at hOne ⊢
    exact hOne

theorem holds_insert {q : QueueBook} {plain : Book} {side : Side} {price : Int}
    {oid qty : Nat} (h : holds q plain)
    (hid : oid ≠ 0) (hqty : 0 < qty) (hfresh : findSlot q.orders oid = none) :
    holds (applyQ (.insert side price oid qty) q)
      (applyPlain (.insert side price oid qty) plain) := by
  rcases h with ⟨hAbs, hBook, hLink⟩
  rcases hBook with ⟨hBid, hAsk, hBidI, hAskI, hBidN, hAskN, hDup, hKeys, hAct⟩
  rcases hAbs with ⟨hBids, hAsks⟩
  have hnotB := not_concat_of_fresh hBidI hfresh
  have hnotA := not_concat_of_fresh hAskI hfresh
  let o : Order := {
    id := oid
    side := side
    price := price
    remaining := qty
    filled := 0
    timeInForce := .gtc
    owner := none
    status := .new
  }
  have hagree : slotAgrees
      (qslot side price qty (insert_order side (queueLevels q side) price oid qty).2) o := by
    exact ⟨rfl, rfl, rfl, rfl⟩
  cases side with
  | buy =>
    refine ⟨?_, ?_, ?_⟩
    · refine ⟨?_, ?_⟩
      · have hplain := toPlainLevels_insert (side := .buy) (levels := q.bids)
          (price := price) (oid := oid) (qty := qty) hid
        simpa [applyQ, applyPlain, add_order, enqueue, writeOrder, abstractsBook,
          setQueueLevels, queueLevels, writeSlot, hBids] using hplain
      · simp [applyQ, applyPlain, add_order, enqueue, writeOrder, abstractsBook,
          setQueueLevels, queueLevels, writeSlot, hAsks]
    · simp only [applyQ, add_order, writeSlot, setQueueLevels, queueLevels]
      refine ⟨?_, hAsk, ?_, ?_, ?_, hAskN, ?_, ?_, ?_⟩
      · exact pairwise_insert (side := .buy) hBid
      · intro level hm
        simpa [qslot] using levelInv_insert hBidI hBid
          hid hqty hfresh hnotB level hm
      · intro level hm
        exact levelInv_upsert_other (hAskI level hm)
          (fun ha => hnotA (mem_concatLive hm ha))
      · intro level hm
        exact live_nonempty_insert hid hBidN level hm
      · exact nodup_concat_insert hid
          (by
            intro hm
            rcases mem_append.mp hm with hb | ha
            · exact hnotB hb
            · exact hnotA ha)
          hDup
      · simpa [qslot] using slotKeys_upsert_nodup hKeys hfresh
      · intro j s hfind hactive
        by_cases hj : j = oid
        · rw [hj] at hfind ⊢
          simp [findSlot_upsert, qslot] at hfind
          cases hfind
          simp [qslot, queueLevels]
          exact oid_mem_insert (side := .buy) hBid hid
        · have hfindOld : findSlot q.orders j = some s := by
            simpa [findSlot_upsert, hj] using hfind
          have hmem := hAct j s hfindOld hactive
          cases hs : s.side with
          | buy =>
            have hmem' : j ∈ liveIds (ordersAt q.bids s.price) := by
              simpa [queueLevels, hs] using hmem
            have hmem2 := mem_ordersAt_insert_old (side := .buy) (price := price) (qty := qty)
              hBid hid hmem'
            simpa [queueLevels, hs] using hmem2
          | sell =>
            simpa [queueLevels, hs] using hmem
    · have hLink1 := linked_upsert (q := q) (plain := enqueue plain o) hLink hfresh rfl hagree
      intro j
      have hOne := hLink1 j
      simp [applyQ, applyPlain, add_order, writeSlot, setQueueLevels, queueLevels,
        enqueue, writeOrder, qslot] at hOne ⊢
      exact hOne
  | sell =>
    refine ⟨?_, ?_, ?_⟩
    · refine ⟨?_, ?_⟩
      · simp [applyQ, applyPlain, add_order, enqueue, writeOrder, abstractsBook,
          setQueueLevels, queueLevels, writeSlot, hBids]
      · have hplain := toPlainLevels_insert (side := .sell) (levels := q.asks)
          (price := price) (oid := oid) (qty := qty) hid
        simpa [applyQ, applyPlain, add_order, enqueue, writeOrder, abstractsBook,
          setQueueLevels, queueLevels, writeSlot, hAsks] using hplain
    · simp only [applyQ, add_order, writeSlot, setQueueLevels, queueLevels]
      refine ⟨hBid, ?_, ?_, ?_, hBidN, ?_, ?_, ?_, ?_⟩
      · exact pairwise_insert (side := .sell) hAsk
      · intro level hm
        exact levelInv_upsert_other (hBidI level hm)
          (fun ha => hnotB (mem_concatLive hm ha))
      · intro level hm
        simpa [qslot] using levelInv_insert hAskI hAsk
          hid hqty hfresh hnotA level hm
      · intro level hm
        exact live_nonempty_insert hid hAskN level hm
      · exact nodup_append_swap (nodup_concat_insert hid
          (by
            intro hm
            rcases mem_append.mp hm with ha | hb
            · exact hnotA ha
            · exact hnotB hb)
          (nodup_append_swap hDup))
      · simpa [qslot] using slotKeys_upsert_nodup hKeys hfresh
      · intro j s hfind hactive
        by_cases hj : j = oid
        · rw [hj] at hfind ⊢
          simp [findSlot_upsert, qslot] at hfind
          cases hfind
          simp [qslot, queueLevels]
          exact oid_mem_insert (side := .sell) hAsk hid
        · have hfindOld : findSlot q.orders j = some s := by
            simpa [findSlot_upsert, hj] using hfind
          have hmem := hAct j s hfindOld hactive
          cases hs : s.side with
          | buy =>
            simpa [queueLevels, hs] using hmem
          | sell =>
            have hmem' : j ∈ liveIds (ordersAt q.asks s.price) := by
              simpa [queueLevels, hs] using hmem
            have hmem2 := mem_ordersAt_insert_old (side := .sell) (price := price) (qty := qty)
              hAsk hid hmem'
            simpa [queueLevels, hs] using hmem2
    · have hLink1 := linked_upsert (q := q) (plain := enqueue plain o) hLink hfresh rfl hagree
      intro j
      have hOne := hLink1 j
      simp [applyQ, applyPlain, add_order, writeSlot, setQueueLevels, queueLevels,
        enqueue, writeOrder, qslot] at hOne ⊢
      exact hOne

theorem nodup_append_parts {l r : List Nat} (hn : (l ++ r).Nodup) :
    l.Nodup ∧ r.Nodup ∧ ∀ a, a ∈ l → a ∉ r := by
  induction l with
  | nil =>
    rw [List.nil_append] at hn
    exact ⟨by simp, hn, fun _ ha => by simp at ha⟩
  | cons head tail ih =>
    rw [cons_append] at hn
    have hn1 := nodup_cons.mp hn
    have hih := ih hn1.2
    refine ⟨nodup_cons.mpr ⟨?_, hih.1⟩, hih.2.1, ?_⟩
    · intro hm
      exact hn1.1 (mem_append.mpr (Or.inl hm))
    · intro a ha hr
      rcases mem_cons.mp ha with heq | ht
      · exact hn1.1 (by simpa [heq] using mem_append.mpr (Or.inr hr))
      · exact hih.2.2 a ht hr

theorem compact_eq {level : QueueLevel}
    (h : level.tombstone_count = zeroCount level.orders) :
    compact level = { level with orders := liveIds level.orders, tombstone_count := 0 } := by
  cases ht : level.tombstone_count == 0
  · simp [compact, ht]
  · have hz : level.tombstone_count = 0 := by simpa using ht
    have hlive : liveIds level.orders = level.orders := liveIds_of_zero (hz ▸ h.symm)
    simp only [compact, ht]
    cases level
    rw [QueueLevel.mk.injEq]
    exact ⟨rfl, hlive.symm, rfl, hz, rfl⟩

theorem liveIds_compact {level : QueueLevel}
    (h : level.tombstone_count = zeroCount level.orders) :
    liveIds (compact level).orders = liveIds level.orders := by
  rw [compact_eq h, liveIds_liveIds]

theorem toPlain_compact {level : QueueLevel}
    (h : level.tombstone_count = zeroCount level.orders) :
    toPlain (compact level) = toPlain level := by
  simp [toPlain, compact_eq h, liveIds_liveIds]

theorem toPlainLevels_compact {levels : List QueueLevel}
    (h : ∀ level ∈ levels, level.tombstone_count = zeroCount level.orders) :
    toPlainLevels (compactLevels levels) = toPlainLevels levels := by
  induction levels with
  | nil => simp [compactLevels, toPlainLevels]
  | cons head tail ih =>
    simp [compactLevels, toPlainLevels, toPlain_compact (h head mem_cons_self),
      ih (fun level hm => h level (mem_cons_of_mem _ hm))]

theorem map_price_compact (levels : List QueueLevel) :
    (compactLevels levels).map (·.price) = levels.map (·.price) := by
  induction levels with
  | nil => simp [compactLevels]
  | cons head tail ih =>
    cases ht : head.tombstone_count == 0 <;> simp [compactLevels, compact, ht, ih]

theorem pairwise_compact {levels : List QueueLevel} {R : Int → Int → Prop}
    (h : levels.Pairwise (fun a b => R a.price b.price)) :
    (compactLevels levels).Pairwise (fun a b => R a.price b.price) := by
  exact pairwise_map.mp (by rw [map_price_compact]; exact pairwise_map.mpr h)

theorem concatLive_compact {levels : List QueueLevel}
    (h : ∀ level ∈ levels, level.tombstone_count = zeroCount level.orders) :
    concatLive (compactLevels levels) = concatLive levels := by
  induction levels with
  | nil => simp [compactLevels, concatLive]
  | cons head tail ih =>
    simp [compactLevels, concatLive, liveIds_compact (h head mem_cons_self),
      ih (fun level hm => h level (mem_cons_of_mem _ hm))]

theorem live_ordersAt_compact {levels : List QueueLevel} {price : Int}
    (h : ∀ level ∈ levels, level.tombstone_count = zeroCount level.orders) :
    liveIds (ordersAt (compactLevels levels) price) = liveIds (ordersAt levels price) := by
  induction levels with
  | nil => simp [compactLevels, ordersAt, liveIds]
  | cons head tail ih =>
    have hc := compact_eq (h head mem_cons_self)
    have htail : ∀ level ∈ tail, level.tombstone_count = zeroCount level.orders :=
      fun level hm => h level (mem_cons_of_mem _ hm)
    cases hp : (head.price == price)
    · simp [compactLevels, ordersAt, hc, hp, ih htail]
    · simp [compactLevels, ordersAt, hc, hp, liveIds_liveIds]

theorem live_nonempty_compact {levels : List QueueLevel}
    (hT : ∀ level ∈ levels, level.tombstone_count = zeroCount level.orders)
    (h : ∀ level ∈ levels, liveIds level.orders ≠ []) :
    ∀ level ∈ compactLevels levels, liveIds level.orders ≠ [] := by
  induction levels with
  | nil => simp [compactLevels]
  | cons head tail ih =>
    have htailT : ∀ level ∈ tail, level.tombstone_count = zeroCount level.orders :=
      fun level hm => hT level (mem_cons_of_mem _ hm)
    have htail : ∀ level ∈ tail, liveIds level.orders ≠ [] :=
      fun level hm => h level (mem_cons_of_mem _ hm)
    intro level hm
    simp [compactLevels] at hm
    rcases hm with heq | hm
    · simpa [heq, liveIds_compact (hT head mem_cons_self)] using h head mem_cons_self
    · exact ih htailT htail level hm

theorem liveSum_setPos (ids : List Nat) (orders : List (Nat × QueueOrder)) (oid pos : Nat) :
    liveSum ids (setPos orders oid pos) = liveSum ids orders := by
  induction ids with
  | nil => simp [liveSum]
  | cons head tail ih =>
    cases head <;> simp [liveSum, remainOf_setPos, ih]

theorem liveSum_rewriteFrom (level : QueueLevel) (ids : List Nat) (offset : Nat)
    (orders : List (Nat × QueueOrder)) (js : List Nat) :
    liveSum js (rewriteLiveFrom level ids offset orders) = liveSum js orders := by
  induction ids generalizing offset orders with
  | nil => simp [rewriteLiveFrom]
  | cons head tail ih =>
    simp [rewriteLiveFrom, ih, liveSum_setPos]

theorem liveSum_rewriteLevels (levels : List QueueLevel) (orders : List (Nat × QueueOrder))
    (js : List Nat) :
    liveSum js (rewriteLevels levels orders) = liveSum js orders := by
  induction levels generalizing orders with
  | nil => simp [rewriteLevels]
  | cons head tail ih =>
    simp [rewriteLevels, rewriteLive, ih, liveSum_rewriteFrom]

theorem slotKeys_rewriteFrom (level : QueueLevel) (ids : List Nat) (offset : Nat)
    (orders : List (Nat × QueueOrder)) :
    slotKeys (rewriteLiveFrom level ids offset orders) = slotKeys orders := by
  induction ids generalizing offset orders with
  | nil => simp [rewriteLiveFrom]
  | cons head tail ih =>
    simp [rewriteLiveFrom, ih, slotKeys_setPos]

theorem slotKeys_rewriteLevels (levels : List QueueLevel) (orders : List (Nat × QueueOrder)) :
    slotKeys (rewriteLevels levels orders) = slotKeys orders := by
  induction levels generalizing orders with
  | nil => simp [rewriteLevels]
  | cons head tail ih =>
    simp [rewriteLevels, rewriteLive, ih, slotKeys_rewriteFrom]

theorem findSlot_rewriteLevels_out {levels : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {oid : Nat} (h : oid ∉ concatLive levels) :
    findSlot (rewriteLevels levels orders) oid = findSlot orders oid := by
  induction levels generalizing orders with
  | nil => simp [rewriteLevels]
  | cons head tail ih =>
    have hhead : oid ∉ liveIds head.orders := by
      intro hm
      exact h (by simpa [concatLive] using mem_append.mpr (Or.inl hm))
    have htail : oid ∉ concatLive tail := by
      intro hm
      exact h (by simpa [concatLive] using mem_append.mpr (Or.inr hm))
    simp [rewriteLevels, rewriteLive, findSlot_rewrite_skip hhead, ih htail]

theorem findSlot_rewriteLevels_at {levels : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {oid : Nat} {s : QueueOrder} {level : QueueLevel}
    (hmem : level ∈ levels) (hid : oid ∈ liveIds level.orders)
    (hn : (concatLive levels).Nodup) (hfind : findSlot orders oid = some s) :
    findSlot (rewriteLevels levels orders) oid =
      some { s with position_in_level :=
        front_position level + idxBefore (liveIds level.orders) oid } := by
  induction levels generalizing orders with
  | nil => cases hmem
  | cons head tail ih =>
    have hcon : (liveIds head.orders ++ concatLive tail).Nodup := by
      simpa [concatLive] using hn
    rcases nodup_append_parts hcon with ⟨hnL, hnR, hdis⟩
    rcases mem_cons.mp hmem with heq | htail
    · subst heq
      have hout : oid ∉ concatLive tail := hdis oid hid
      have hstep := findSlot_rewrite_mem (level := level) (offset := 0) hfind hid hnL
      simp [rewriteLevels, rewriteLive, findSlot_rewriteLevels_out hout, hstep, Nat.add_zero]
    · have hnot : oid ∉ liveIds head.orders := by
        intro hm
        exact hdis oid hm (mem_concatLive htail hid)
      have hfind' : findSlot (rewriteLive head orders) oid = some s := by
        simpa [rewriteLive, findSlot_rewrite_skip hnot] using hfind
      simpa [rewriteLevels] using ih htail (by simpa [concatLive] using hnR) hfind'

theorem levelInv_skip_rewriteLive {level other : QueueLevel}
    {orders : List (Nat × QueueOrder)} {side : Side}
    (h : levelInv level orders side)
    (hdis : ∀ a ∈ liveIds level.orders, a ∉ liveIds other.orders) :
    levelInv level (rewriteLive other orders) side := by
  rcases h with ⟨hTomb, hTot, hEmpty, hLive⟩
  refine ⟨hTomb, ?_, hEmpty, ?_⟩
  · have hsum := liveSum_rewriteFrom other (liveIds other.orders) 0 orders level.orders
    simpa [rewriteLive, hsum] using hTot
  · intro oid hm
    rcases hLive oid hm with ⟨s, hfind, hact, hrem, hside, hprice, hpop, hget⟩
    have hsame := findSlot_rewrite_skip (level := other) (orders := orders) (offset := 0)
      (hdis oid hm)
    refine ⟨s, ?_, hact, hrem, hside, hprice, hpop, hget⟩
    simpa [rewriteLive, hsame] using hfind

theorem levelInv_stable_out {level : QueueLevel} {levels : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {side : Side}
    (h : levelInv level orders side)
    (hdis : ∀ a ∈ liveIds level.orders, a ∉ concatLive levels) :
    levelInv level (rewriteLevels levels orders) side := by
  induction levels generalizing orders with
  | nil => simpa [rewriteLevels] using h
  | cons head tail ih =>
    have hhead : ∀ a ∈ liveIds level.orders, a ∉ liveIds head.orders := by
      intro a ha hm
      exact hdis a ha (by simpa [concatLive] using mem_append.mpr (Or.inl hm))
    have htail : ∀ a ∈ liveIds level.orders, a ∉ concatLive tail := by
      intro a ha hm
      exact hdis a ha (by simpa [concatLive] using mem_append.mpr (Or.inr hm))
    simpa [rewriteLevels] using ih (levelInv_skip_rewriteLive h hhead) htail

theorem levelInv_compact_one {level : QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} (h : levelInv level orders side) (hn : (liveIds level.orders).Nodup) :
    levelInv (compact level) (rewriteLive (compact level) orders) side := by
  rcases h with ⟨hTomb, hTot, hEmpty, hLive⟩
  have hceq := compact_eq hTomb
  have hlive := liveIds_compact hTomb
  refine ⟨?_, ?_, ?_, ?_⟩
  · rw [hceq]
    simp [zeroCount_liveIds]
  · have horders : (compact level).orders = liveIds level.orders := by simp [hceq]
    have htot : (compact level).total_quantity = level.total_quantity := by simp [hceq]
    have hids : liveIds (compact level).orders = liveIds level.orders := hlive
    have hsum :=
      liveSum_rewriteFrom (compact level) (liveIds level.orders) 0 orders (liveIds level.orders)
    rw [rewriteLive, hids, horders, htot, hsum, liveSum_liveIds]
    exact hTot
  · rw [hceq]
    simp [is_empty, liveIds_liveIds] at hEmpty ⊢
    exact hEmpty
  · intro oid hm
    have hm0 : oid ∈ liveIds level.orders := by simpa [hlive] using hm
    rcases hLive oid hm0 with ⟨s, hfind, hact, hrem, hside, hprice, hpop, hget⟩
    have hids : liveIds (compact level).orders = liveIds level.orders := hlive
    have hmem : oid ∈ liveIds (compact level).orders := by simpa [hids] using hm0
    have hn' : (liveIds (compact level).orders).Nodup := by simpa [hids] using hn
    have hstep := findSlot_rewrite_mem (level := compact level) (offset := 0)
      (ids := liveIds (compact level).orders) hfind hmem hn'
    let pos : Nat :=
      front_position (compact level) + idxBefore (liveIds (compact level).orders) oid
    let s' : QueueOrder := { s with position_in_level := pos }
    refine ⟨s', ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
    · simpa [rewriteLive, Nat.add_zero] using hstep
    · simpa [s'] using hact
    · simpa [s'] using hrem
    · simpa [s'] using hside
    · rw [hceq]
      simpa [s'] using hprice
    · simp only [s', pos, front_position, hceq]
      omega
    · have hpos : s'.position_in_level =
          front_position (compact level) + idxBefore (liveIds (compact level).orders) oid := by
        simp [s', pos]
      rw [hceq] at hpos ⊢
      simp only [front_position, liveIds_liveIds] at hpos
      have hidx : s'.position_in_level - level.popped_from_front =
          idxBefore (liveIds level.orders) oid := by
        rw [hpos]
        omega
      simpa [hidx] using getAt_idxBefore hm0 hn

theorem levelsInv_compact {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hn : (concatLive levels).Nodup) :
    ∀ level ∈ compactLevels levels,
      levelInv level (rewriteLevels (compactLevels levels) orders) side := by
  induction levels generalizing orders with
  | nil => simp [compactLevels]
  | cons head tail ih =>
    intro level hm
    have hhead := hInv head mem_cons_self
    have htailInv : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hcon : (liveIds head.orders ++ concatLive tail).Nodup := by
      simpa [concatLive] using hn
    rcases nodup_append_parts hcon with ⟨hnL, hnR, hdis⟩
    have htailT : ∀ level ∈ tail, level.tombstone_count = zeroCount level.orders :=
      fun level hm => (htailInv level hm).1
    simp [compactLevels] at hm
    rcases hm with heq | hm
    · subst heq
      have hHere := levelInv_compact_one hhead hnL
      have hskip : ∀ a ∈ liveIds (compact head).orders,
          a ∉ concatLive (compactLevels tail) := by
        intro a ha hmem
        have ha' : a ∈ liveIds head.orders := by
          simpa [liveIds_compact hhead.1] using ha
        have hmem' : a ∈ concatLive tail := by
          simpa [concatLive_compact htailT] using hmem
        exact hdis a ha' hmem'
      simpa [compactLevels, rewriteLevels] using levelInv_stable_out hHere hskip
    · have hInv2 : ∀ level ∈ tail,
          levelInv level (rewriteLive (compact head) orders) side := by
        intro level hl
        have hskip : ∀ a ∈ liveIds level.orders, a ∉ liveIds (compact head).orders := by
          intro a ha hmem
          have hmem' : a ∈ liveIds head.orders := by
            simpa [liveIds_compact hhead.1] using hmem
          exact hdis a hmem' (mem_concatLive hl ha)
        exact levelInv_skip_rewriteLive (htailInv level hl) hskip
      simpa [compactLevels, rewriteLevels] using ih hInv2 hnR level hm

theorem levelsInv_compact_outer {levels other : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {side : Side}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hn : (concatLive levels).Nodup)
    (hdis : ∀ a ∈ concatLive levels, a ∉ concatLive other)
    (hT : ∀ level ∈ other, level.tombstone_count = zeroCount level.orders) :
    ∀ level ∈ compactLevels levels,
      levelInv level
        (rewriteLevels (compactLevels other) (rewriteLevels (compactLevels levels) orders)) side := by
  intro level hm
  have hHere := levelsInv_compact hInv hn level hm
  have hskip : ∀ a ∈ liveIds level.orders, a ∉ concatLive (compactLevels other) := by
    intro a ha hmem
    have hside : a ∈ concatLive levels := by
      have hback : level ∈ compactLevels levels := hm
      have ha' : a ∈ concatLive (compactLevels levels) := mem_concatLive hback ha
      simpa [concatLive_compact (fun l hl => (hInv l hl).1)] using ha'
    exact hdis a hside (by simpa [concatLive_compact hT] using hmem)
  exact levelInv_stable_out hHere hskip

theorem findSlot_rewriteFrom_none {level : QueueLevel} {ids : List Nat} {offset : Nat}
    {orders : List (Nat × QueueOrder)} {j : Nat} (h : findSlot orders j = none) :
    findSlot (rewriteLiveFrom level ids offset orders) j = none := by
  induction ids generalizing offset orders with
  | nil => simp [rewriteLiveFrom, h]
  | cons head tail ih =>
    by_cases heq : head = j
    · subst heq
      have hset := findSlot_setPos_none (pos := front_position level + offset) h
      simpa [rewriteLiveFrom] using ih (offset := offset + 1) hset
    · have hset : findSlot (setPos orders head (front_position level + offset)) j = none := by
        simpa [findSlot_setPos_ne heq] using h
      simpa [rewriteLiveFrom] using ih (offset := offset + 1) hset

theorem findSlot_rewriteLevels_none {levels : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {j : Nat} (h : findSlot orders j = none) :
    findSlot (rewriteLevels levels orders) j = none := by
  induction levels generalizing orders with
  | nil => simp [rewriteLevels, h]
  | cons head tail ih =>
    simpa [rewriteLevels, rewriteLive] using
      ih (findSlot_rewriteFrom_none (ids := liveIds head.orders) (offset := 0) h)

theorem findSlot_setPos_fields {orders : List (Nat × QueueOrder)} {oid pos j : Nat}
    {s : QueueOrder} (h : findSlot (setPos orders oid pos) j = some s) :
    ∃ s0, findSlot orders j = some s0 ∧ s.side = s0.side ∧ s.price = s0.price ∧
      s.remaining = s0.remaining ∧ s.active = s0.active := by
  induction orders with
  | nil => simp [setPos, findSlot] at h
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid
    · by_cases hj : j = k
      · have hs : s = { prev with position_in_level := pos } := by
          simpa [setPos, findSlot, hk, hj] using h.symm
        refine ⟨prev, by simp [findSlot, hj], by simp [hs], by simp [hs], by simp [hs], by simp [hs]⟩
      · have htail : findSlot tail j = some s := by
          have hne : oid ≠ j := by
            intro heq
            exact hj (heq.symm.trans hk.symm)
          simpa [setPos, findSlot, hk, hne] using h
        exact ⟨s, by simp [findSlot, Ne.symm hj, htail], rfl, rfl, rfl, rfl⟩
    · by_cases hj : j = k
      · have hs : s = prev := by
          simpa [setPos, findSlot, hk, hj] using h.symm
        refine ⟨prev, by simp [findSlot, hj], by simp [hs], by simp [hs], by simp [hs], by simp [hs]⟩
      · have hrec : findSlot (setPos tail oid pos) j = some s := by
          simpa [setPos, findSlot, hk, Ne.symm hj] using h
        rcases ih hrec with ⟨s0, h0, hside, hprice, hrem, hact⟩
        refine ⟨s0, by simp [findSlot, Ne.symm hj, h0], hside, hprice, hrem, hact⟩

theorem findSlot_rewriteFrom_fields {level : QueueLevel} {ids : List Nat} {offset : Nat}
    {orders : List (Nat × QueueOrder)} {j : Nat} {s : QueueOrder}
    (h : findSlot (rewriteLiveFrom level ids offset orders) j = some s) :
    ∃ s0, findSlot orders j = some s0 ∧ s.side = s0.side ∧ s.price = s0.price ∧
      s.remaining = s0.remaining ∧ s.active = s0.active := by
  induction ids generalizing offset orders s with
  | nil =>
    simp [rewriteLiveFrom] at h
    exact ⟨s, h, rfl, rfl, rfl, rfl⟩
  | cons head tail ih =>
    simp [rewriteLiveFrom] at h
    rcases ih h with ⟨s1, h1, hside, hprice, hrem, hact⟩
    rcases findSlot_setPos_fields h1 with ⟨s0, h0, hside0, hprice0, hrem0, hact0⟩
    exact ⟨s0, h0, hside.trans hside0, hprice.trans hprice0, hrem.trans hrem0, hact.trans hact0⟩

theorem findSlot_rewriteLevels_fields {levels : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {j : Nat} {s : QueueOrder}
    (h : findSlot (rewriteLevels levels orders) j = some s) :
    ∃ s0, findSlot orders j = some s0 ∧ s.side = s0.side ∧ s.price = s0.price ∧
      s.remaining = s0.remaining ∧ s.active = s0.active := by
  induction levels generalizing orders s with
  | nil =>
    simp [rewriteLevels] at h
    exact ⟨s, h, rfl, rfl, rfl, rfl⟩
  | cons head tail ih =>
    simp [rewriteLevels] at h
    rcases ih h with ⟨s1, h1, hside, hprice, hrem, hact⟩
    rcases findSlot_rewriteFrom_fields (ids := liveIds head.orders) (offset := 0) h1 with
      ⟨s0, h0, hside0, hprice0, hrem0, hact0⟩
    exact ⟨s0, h0, hside.trans hside0, hprice.trans hprice0, hrem.trans hrem0, hact.trans hact0⟩

theorem findSlot_rewriteFrom_some {level : QueueLevel} {ids : List Nat} {offset : Nat}
    {orders : List (Nat × QueueOrder)} {j : Nat}
    (h : ∃ s, findSlot orders j = some s) :
    ∃ s', findSlot (rewriteLiveFrom level ids offset orders) j = some s' := by
  induction ids generalizing offset orders with
  | nil =>
    simpa [rewriteLiveFrom] using h
  | cons head tail ih =>
    rcases h with ⟨s, h⟩
    by_cases heq : head = j
    · subst heq
      have hset := findSlot_setPos_self (pos := front_position level + offset) h
      let s' : QueueOrder := { s with position_in_level := front_position level + offset }
      simpa [rewriteLiveFrom] using ih (offset := offset + 1) ⟨s', hset⟩
    · have hset : findSlot (setPos orders head (front_position level + offset)) j = some s := by
        simpa [findSlot_setPos_ne heq] using h
      simpa [rewriteLiveFrom] using ih (offset := offset + 1) ⟨s, hset⟩

theorem findSlot_rewriteLevels_some {levels : List QueueLevel}
    {orders : List (Nat × QueueOrder)} {j : Nat}
    (h : ∃ s, findSlot orders j = some s) :
    ∃ s', findSlot (rewriteLevels levels orders) j = some s' := by
  induction levels generalizing orders with
  | nil =>
    simpa [rewriteLevels] using h
  | cons head tail ih =>
    rcases h with ⟨s, h⟩
    have h1 := findSlot_rewriteFrom_some (level := head) (ids := liveIds head.orders)
      (offset := 0) ⟨s, h⟩
    simpa [rewriteLevels, rewriteLive] using ih h1

theorem holds_compact {q : QueueBook} {plain : Book} (h : holds q plain) :
    holds (applyQ .compact q) (applyPlain .compact plain) := by
  rcases h with ⟨hAbs, hBook, hLink⟩
  rcases hBook with ⟨hBid, hAsk, hBidI, hAskI, hBidN, hAskN, hDup, hKeys, hAct⟩
  rcases hAbs with ⟨hBids, hAsks⟩
  have hBidT : ∀ level ∈ q.bids, level.tombstone_count = zeroCount level.orders :=
    fun level hm => (hBidI level hm).1
  have hAskT : ∀ level ∈ q.asks, level.tombstone_count = zeroCount level.orders :=
    fun level hm => (hAskI level hm).1
  have hcon := nodup_append_parts hDup
  refine ⟨?_, ?_, ?_⟩
  · simp [applyQ, applyPlain, compact_book, abstractsBook, hBids, hAsks,
      toPlainLevels_compact hBidT, toPlainLevels_compact hAskT]
  · simp only [applyQ, compact_book]
    refine ⟨pairwise_compact hBid, pairwise_compact hAsk, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
    · exact levelsInv_compact_outer hBidI hcon.1 hcon.2.2 hAskT
    · intro level hm
      have hInvAsk : ∀ level ∈ q.asks,
          levelInv level (rewriteLevels (compactLevels q.bids) q.orders) .sell := by
        intro level hl
        have hskip : ∀ a ∈ liveIds level.orders, a ∉ concatLive (compactLevels q.bids) := by
          intro a ha hmem
          exact hcon.2.2 a (by simpa [concatLive_compact hBidT] using hmem) (mem_concatLive hl ha)
        exact levelInv_stable_out (hAskI level hl) hskip
      have hnAsk : (concatLive q.asks).Nodup := hcon.2.1
      exact levelsInv_compact hInvAsk hnAsk level hm
    · exact live_nonempty_compact hBidT hBidN
    · exact live_nonempty_compact hAskT hAskN
    · simpa [concatLive_compact hBidT, concatLive_compact hAskT] using hDup
    · simpa [slotKeys_rewriteLevels] using hKeys
    · intro j s hfind hactive
      rcases findSlot_rewriteLevels_fields hfind with ⟨s1, h1, hside1, hprice1, hrem1, hact1⟩
      rcases findSlot_rewriteLevels_fields h1 with ⟨s0, h0, hside0, hprice0, hrem0, hact0⟩
      have hact0 : s0.active = true := by
        simpa [hact1, hact0] using hactive
      have hmem := hAct j s0 h0 hact0
      cases hside : s.side with
      | buy =>
        have hside0' : s0.side = .buy := by
          simpa [hside] using hside0.symm.trans hside1.symm
        have hprice0' : s0.price = s.price := hprice0.symm.trans hprice1.symm
        have hmem' : j ∈ liveIds (ordersAt q.bids s.price) := by
          simpa [queueLevels, hside0', hprice0'] using hmem
        have hmem2 := live_ordersAt_compact (levels := q.bids) (price := s.price) hBidT
        simpa [queueLevels, hside, ← hmem2] using hmem'
      | sell =>
        have hside0' : s0.side = .sell := by
          simpa [hside] using hside0.symm.trans hside1.symm
        have hprice0' : s0.price = s.price := hprice0.symm.trans hprice1.symm
        have hmem' : j ∈ liveIds (ordersAt q.asks s.price) := by
          simpa [queueLevels, hside0', hprice0'] using hmem
        have hmem2 := live_ordersAt_compact (levels := q.asks) (price := s.price) hAskT
        simpa [queueLevels, hside, ← hmem2] using hmem'
  · intro j
    have hOne := hLink j
    cases hOld : findSlot q.orders j with
    | none =>
      have hNew := findSlot_rewriteLevels_none
        (levels := compactLevels q.asks)
        (findSlot_rewriteLevels_none (levels := compactLevels q.bids) hOld)
      simp [applyQ, applyPlain, compact_book, hOld, hNew] at hOne ⊢
      exact hOne
    | some s0 =>
      simp [applyQ, applyPlain, compact_book] at hOne ⊢
      cases hNew : findSlot (rewriteLevels (compactLevels q.asks)
          (rewriteLevels (compactLevels q.bids) q.orders)) j with
      | none =>
        rcases findSlot_rewriteLevels_some (levels := compactLevels q.bids) ⟨s0, hOld⟩ with ⟨_, h1⟩
        rcases findSlot_rewriteLevels_some (levels := compactLevels q.asks) ⟨_, h1⟩ with ⟨_, h2⟩
        simp [hNew] at h2
      | some s =>
        rcases findSlot_rewriteLevels_fields hNew with ⟨s1, h1, hside1, hprice1, hrem1, hact1⟩
        rcases findSlot_rewriteLevels_fields h1 with ⟨s0', h0, hside0, hprice0, hrem0, hact0⟩
        simp [hOld] at h0
        cases h0
        simp [hOld] at hOne
        cases hfo : findOrder plain.orders j with
        | none => simp [hfo] at hOne
        | some o =>
          simp [hfo] at hOne
          refine ⟨hOne.1, ?_, ?_, ?_, ?_⟩
          · exact hside0.symm.trans hside1.symm ▸ hOne.2.1
          · exact hprice0.symm.trans hprice1.symm ▸ hOne.2.2.1
          · exact hrem0.symm.trans hrem1.symm ▸ hOne.2.2.2.1
          · exact hact0.symm.trans hact1.symm ▸ hOne.2.2.2.2

def slotOff (s : QueueOrder) : QueueOrder :=
  { s with remaining := 0, active := false }

theorem list_isEmpty_true {l : List Nat} : l.isEmpty = true ↔ l = [] := by
  cases l <;> simp

theorem slotKeys_upsert_existing {orders : List (Nat × QueueOrder)} {oid : Nat}
    {o s : QueueOrder} (h : findSlot orders oid = some s) :
    slotKeys (upsertSlot orders oid o) = slotKeys orders := by
  induction orders with
  | nil => simp [findSlot] at h
  | cons entry tail ih =>
    rcases entry with ⟨k, prev⟩
    by_cases hk : k = oid
    · simp [upsertSlot, slotKeys, hk]
    · simp [findSlot, hk] at h
      simp [upsertSlot, slotKeys, hk, ih h]

theorem liveSum_zero_iff {ids : List Nat} {orders : List (Nat × QueueOrder)}
    (hpos : ∀ a, a ∈ liveIds ids → 0 < remainOf orders a) :
    liveSum ids orders = 0 ↔ liveIds ids = [] := by
  induction ids with
  | nil => simp [liveSum, liveIds]
  | cons head tail ih =>
    cases head with
    | zero =>
      simp [liveSum, liveIds]
      exact ih fun a ha => hpos a (by simp [liveIds, ha])
    | succ n =>
      have hlt : 0 < remainOf orders (n + 1) := hpos (n + 1) (by simp [liveIds])
      constructor
      · intro h
        simp [liveSum] at h
        omega
      · intro h
        simp [liveIds] at h

theorem liveSum_mem_le {ids : List Nat} {orders : List (Nat × QueueOrder)} {a : Nat}
    (hm : a ∈ liveIds ids) : remainOf orders a ≤ liveSum ids orders := by
  induction ids with
  | nil => simp [liveIds] at hm
  | cons head tail ih =>
    cases head with
    | zero =>
      simp [liveSum, liveIds] at hm ⊢
      exact ih hm
    | succ n =>
      simp [liveSum, liveIds] at hm ⊢
      rcases hm with heq | hm
      · simp [heq]
      · have ih1 := ih hm
        omega

theorem mem_removeId_iff_of_ne {ids : List Nat} {a j : Nat} (hne : j ≠ a) :
    j ∈ removeId ids a ↔ j ∈ ids := by
  induction ids with
  | nil => simp [removeId]
  | cons head tail ih =>
    by_cases hh : head = a
    · subst hh
      simp [removeId, hne]
    · have hb : (head == a) = false := by simpa using hh
      simp [removeId, hb, ih]

theorem removeId_append_left {l r : List Nat} {a : Nat} (h : a ∉ r) :
    removeId (l ++ r) a = removeId l a ++ r := by
  induction l with
  | nil => simp [List.nil_append, removeId, removeId_of_not_mem h]
  | cons head tail ih =>
    by_cases hh : head = a
    · subst hh
      simp [removeId, cons_append]
    · have hb : (head == a) = false := by simpa using hh
      simp [removeId, cons_append, hb, ih]

theorem removeId_append_right {l r : List Nat} {a : Nat} (h : a ∉ l) :
    removeId (l ++ r) a = l ++ removeId r a := by
  induction l with
  | nil => simp [removeId]
  | cons head tail ih =>
    have hne : head ≠ a := by
      intro heq
      exact h (heq ▸ mem_cons_self)
    have ht : a ∉ tail := fun hm => h (mem_cons_of_mem _ hm)
    have hb : (head == a) = false := by simpa using hne
    simp [removeId, cons_append, hb, ih ht]

theorem mem_ordersAt_concat {levels : List QueueLevel} {price : Int} {a : Nat}
    (h : a ∈ liveIds (ordersAt levels price)) : a ∈ concatLive levels := by
  induction levels with
  | nil => simp [ordersAt, liveIds] at h
  | cons head tail ih =>
    cases hp : (head.price == price)
    · simp [ordersAt, hp] at h
      simpa [concatLive] using mem_append.mpr (Or.inr (ih h))
    · simp [ordersAt, hp] at h
      simpa [concatLive] using mem_append.mpr (Or.inl h)

theorem ordersAt_tail_ne {head : QueueLevel} {tail : List QueueLevel} {price : Int}
    (h : (head :: tail).Pairwise (fun a b => a.price ≠ b.price))
    (hp : head.price = price) : ordersAt tail price = [] := by
  induction tail with
  | nil => simp [ordersAt]
  | cons x xs ih =>
    have hpair := pairwise_cons.mp h
    have hne : x.price ≠ price := Ne.symm (by simpa [hp] using hpair.1 x mem_cons_self)
    have hb : (x.price == price) = false := by simpa using hne
    have htail : (head :: xs).Pairwise (fun a b => a.price ≠ b.price) := by
      refine pairwise_cons.mpr ⟨?_, ?_⟩
      · intro a ha
        exact hpair.1 a (mem_cons_of_mem _ ha)
      · exact (pairwise_cons.mp hpair.2).2
    simp [ordersAt, hb, ih htail]

theorem mark_tombstone_live {level : QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {oid : Nat} {s : QueueOrder}
    (h : levelInv level orders side) (hm : oid ∈ liveIds level.orders)
    (hfind : findSlot orders oid = some s) :
    mark_tombstone level s.position_in_level s.remaining =
      { level with
        orders := setAt level.orders (s.position_in_level - level.popped_from_front) 0
        total_quantity := level.total_quantity - s.remaining
        tombstone_count := level.tombstone_count + 1 } := by
  rcases h with ⟨_, _, _, hLive⟩
  rcases hLive oid hm with ⟨s0, hfind0, _, _, _, _, hpop, hget⟩
  have he : some s0 = some s := by rw [← hfind0, hfind]
  injection he with hs
  rw [hs] at hpop hget
  have hnz : oid ≠ 0 := (mem_liveIds.mp hm).1
  have hlt : ¬ s.position_in_level < level.popped_from_front := by
    intro hbad
    omega
  have hb : (oid == 0) = false := by simpa using hnz
  simp [mark_tombstone, hlt, hget, hb]

theorem levelInv_marked {level : QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {oid : Nat} {s : QueueOrder}
    (h : levelInv level orders side) (hn : (liveIds level.orders).Nodup)
    (hm : oid ∈ liveIds level.orders) (hfind : findSlot orders oid = some s) :
    levelInv (mark_tombstone level s.position_in_level s.remaining)
      (upsertSlot orders oid (slotOff s)) side := by
  have hshape := mark_tombstone_live h hm hfind
  rcases h with ⟨hTomb, hTot, _, hLive⟩
  rcases hLive oid hm with ⟨s0, hfind0, _, _, _, _, _, hget⟩
  have he : some s0 = some s := by rw [← hfind0, hfind]
  injection he with hs
  rw [hs] at hget
  have hnz : oid ≠ 0 := (mem_liveIds.mp hm).1
  rw [hshape]
  let idx : Nat := s.position_in_level - level.popped_from_front
  let ids' : List Nat := setAt level.orders idx 0
  let orders' : List (Nat × QueueOrder) := upsertSlot orders oid (slotOff s)
  have hids : liveIds ids' = removeId (liveIds level.orders) oid :=
    liveIds_setAt_zero hget hnz hn
  have hnot : oid ∉ liveIds ids' := not_mem_live_setAt hget hnz hn
  have hskip : liveSum ids' orders' = liveSum ids' orders := liveSum_upsert_skip hnot
  have hrem : remainOf orders oid = s.remaining := by simp [remainOf, hfind]
  have hsplit := liveSum_setAt_add (orders := orders) hget hnz hn
  have hAdd : liveSum ids' orders + s.remaining = level.total_quantity := by
    simpa [hrem, hTot] using hsplit
  have hsub : level.total_quantity - s.remaining = liveSum ids' orders := by
    rw [← hAdd]
    exact Nat.add_sub_cancel _ _
  refine ⟨?_, ?_, ?_, ?_⟩
  · have hz := zeroCount_setAt_zero hget hnz
    simp [hTomb, hz]
  · rw [hskip]
    exact hsub
  · have hpos : ∀ a, a ∈ liveIds ids' → 0 < remainOf orders' a := by
      intro a ha
      rw [hids] at ha
      have hne : a ≠ oid := by
        intro heqa
        exact removeId_not_mem hn oid (heqa ▸ ha)
      have ha0 : a ∈ liveIds level.orders := (mem_removeId_iff_of_ne hne).mp ha
      rcases hLive a ha0 with ⟨s1, hfind1, _, hrem1, _, _, _, _⟩
      simpa [orders', remainOf, findSlot_upsert, hne, hfind1] using hrem1
    have hiff := liveSum_zero_iff hpos
    simp only [is_empty, beq_iff_eq, hsub, ids', idx]
    rw [← hskip]
    exact hiff
  · intro a ha
    have ha' : a ∈ removeId (liveIds level.orders) oid := by
      simpa [ids', idx, hids] using ha
    have hne : a ≠ oid := by
      intro heqa
      exact removeId_not_mem hn oid (heqa ▸ ha')
    have ha0 : a ∈ liveIds level.orders := (mem_removeId_iff_of_ne hne).mp ha'
    rcases hLive a ha0 with ⟨s1, hfind1, hact1, hrem1, hside1, hprice1, hpop1, hget1⟩
    refine ⟨s1, ?_, hact1, hrem1, hside1, hprice1, hpop1, ?_⟩
    · simpa [orders', findSlot_upsert, hne] using hfind1
    · have hidx : idx ≠ s1.position_in_level - level.popped_from_front := by
        intro heq
        have hsame : getAt level.orders (s1.position_in_level - level.popped_from_front) =
            some oid := by simpa [idx, heq] using hget
        have hao : some a = some oid := by rw [← hget1, hsame]
        injection hao with hao
        exact hne hao
      simpa [idx] using getAt_setAt_ne hidx hget1

theorem pairwise_gt_ne {levels : List QueueLevel}
    (h : levels.Pairwise (fun a b => a.price > b.price)) :
    levels.Pairwise (fun a b => a.price ≠ b.price) := by
  induction levels with
  | nil => simp
  | cons head tail ih =>
    have hpair := pairwise_cons.mp h
    refine pairwise_cons.mpr ⟨fun b hb => ?_, ih hpair.2⟩
    intro heq
    have hab := hpair.1 b hb
    rw [heq] at hab
    omega

theorem pairwise_lt_ne {levels : List QueueLevel}
    (h : levels.Pairwise (fun a b => a.price < b.price)) :
    levels.Pairwise (fun a b => a.price ≠ b.price) := by
  induction levels with
  | nil => simp
  | cons head tail ih =>
    have hpair := pairwise_cons.mp h
    refine pairwise_cons.mpr ⟨fun b hb => ?_, ih hpair.2⟩
    intro heq
    have hab := hpair.1 b hb
    rw [heq] at hab
    omega

theorem mark_tombstone_price (level : QueueLevel) (position quantity : Nat) :
    (mark_tombstone level position quantity).price = level.price := by
  unfold mark_tombstone
  by_cases hlt : position < level.popped_from_front
  · simp [hlt]
  · simp [hlt]
    split
    · rfl
    · split <;> rfl

theorem mem_price_mark_at {levels : List QueueLevel} {price : Int} {pos qty : Nat}
    {level : QueueLevel} (hm : level ∈ mark_at levels price pos qty) :
    ∃ orig, orig ∈ levels ∧ orig.price = level.price := by
  induction levels with
  | nil => simp [mark_at] at hm
  | cons head tail ih =>
    cases hp : (head.price == price)
    · simp [mark_at, hp] at hm
      rcases hm with heq | hm
      · exact ⟨head, mem_cons_self, by simpa [heq]⟩
      · rcases ih hm with ⟨orig, ho, heq⟩
        exact ⟨orig, mem_cons_of_mem _ ho, heq⟩
    · cases hempty : is_empty (mark_tombstone head pos qty)
      · simp [mark_at, hp, hempty] at hm
        rcases hm with heq | hm
        · exact ⟨head, mem_cons_self, by
            simpa [heq] using (mark_tombstone_price head pos qty).symm⟩
        · exact ⟨level, mem_cons_of_mem _ hm, rfl⟩
      · simp [mark_at, hp, hempty] at hm
        exact ⟨level, mem_cons_of_mem _ hm, rfl⟩

theorem pairwise_mark_at {levels : List QueueLevel} {price : Int} {pos qty : Nat}
    {R : Int → Int → Prop}
    (h : levels.Pairwise (fun a b => R a.price b.price)) :
    (mark_at levels price pos qty).Pairwise (fun a b => R a.price b.price) := by
  induction levels with
  | nil => simp [mark_at]
  | cons head tail ih =>
    have hpair := pairwise_cons.mp h
    cases hp : (head.price == price)
    · have hshape : mark_at (head :: tail) price pos qty =
          head :: mark_at tail price pos qty := by simp [mark_at, hp]
      rw [hshape]
      refine pairwise_cons.mpr ⟨?_, ih hpair.2⟩
      intro level hm
      rcases mem_price_mark_at hm with ⟨orig, ho, heq⟩
      simpa [heq] using hpair.1 orig ho
    · cases hempty : is_empty (mark_tombstone head pos qty)
      · have hpr := mark_tombstone_price head pos qty
        simp [mark_at, hp, hempty]
        refine ⟨fun level hm => by simpa [hpr] using hpair.1 level hm, hpair.2⟩
      · simpa [mark_at, hp, hempty] using hpair.2

theorem not_mem_live_level {levels : List QueueLevel} {level : QueueLevel} {a : Nat}
    (h : a ∉ concatLive levels) (hm : level ∈ levels) : a ∉ liveIds level.orders :=
  fun ha => h (mem_concatLive hm ha)

theorem mem_of_remove {ids : List Nat} {a j : Nat} (h : j ∈ removeId ids a) : j ∈ ids := by
  induction ids with
  | nil => simp [removeId] at h
  | cons head tail ih =>
    by_cases hh : head = a
    · subst hh
      simp [removeId] at h
      exact mem_cons_of_mem _ h
    · have hb : (head == a) = false := by simpa using hh
      simp [removeId, hb] at h
      rcases h with heq | ht
      · exact heq ▸ mem_cons_self
      · exact mem_cons_of_mem _ (ih ht)

theorem nodup_remove_append {l r : List Nat} {a : Nat} (hn : (l ++ r).Nodup) :
    (removeId l a ++ r).Nodup := by
  induction l with
  | nil =>
    simp [removeId] at hn ⊢
    exact hn
  | cons head tail ih =>
    rw [cons_append] at hn
    have hn1 := nodup_cons.mp hn
    by_cases hh : head = a
    · subst hh
      simp [removeId]
      exact hn1.2
    · have hb : (head == a) = false := by simpa using hh
      have heq : removeId (head :: tail) a = head :: removeId tail a := by
        simp [removeId, hb]
      rw [heq, cons_append]
      refine nodup_cons.mpr ⟨?_, ih hn1.2⟩
      intro hm
      rcases mem_append.mp hm with hml | hmr
      · exact hn1.1 (mem_append.mpr (Or.inl (mem_of_remove hml)))
      · exact hn1.1 (mem_append.mpr (Or.inr hmr))

theorem nodup_append_remove {l r : List Nat} {a : Nat} (hn : (l ++ r).Nodup) :
    (l ++ removeId r a).Nodup := by
  exact nodup_append_swap (nodup_remove_append (nodup_append_swap hn))

theorem levelsInv_mark {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid : Nat} {s : QueueOrder}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hPair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hDup : (concatLive levels).Nodup)
    (hmem : oid ∈ liveIds (ordersAt levels price))
    (hfind : findSlot orders oid = some s) :
    ∀ level ∈ mark_at levels price s.position_in_level s.remaining,
      levelInv level (upsertSlot orders oid (slotOff s)) side := by
  induction levels with
  | nil => simp [ordersAt, liveIds] at hmem
  | cons head tail ih =>
    have hhead := hInv head mem_cons_self
    have htailI : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hDup)
    have hpair := pairwise_cons.mp hPair
    cases hp : (head.price == price)
    · have hmemt : oid ∈ liveIds (ordersAt tail price) := by
        simpa [ordersAt, hp] using hmem
      have hnot : oid ∉ liveIds head.orders := by
        intro ha
        exact hparts.2.2 oid ha (mem_ordersAt_concat hmemt)
      intro level hm
      simp [mark_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using levelInv_upsert_other hhead hnot
      · exact ih htailI hpair.2 hparts.2.1 hmemt level hm
    · have hprice : head.price = price := by simpa using hp
      have hmemh : oid ∈ liveIds head.orders := by
        simpa [ordersAt, hp] using hmem
      have hn : (liveIds head.orders).Nodup := hparts.1
      have hmarked := levelInv_marked hhead hn hmemh hfind
      have hnottail : oid ∉ concatLive tail := hparts.2.2 oid hmemh
      cases hempty : is_empty (mark_tombstone head s.position_in_level s.remaining)
      · intro level hm
        simp [mark_at, hp, hempty] at hm
        rcases hm with heq | hm
        · simpa [heq] using hmarked
        · exact levelInv_upsert_other (htailI level hm)
            (not_mem_live_level hnottail hm)
      · intro level hm
        simp [mark_at, hp, hempty] at hm
        exact levelInv_upsert_other (htailI level hm) (not_mem_live_level hnottail hm)

theorem toPlainLevels_mark {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid : Nat} {s : QueueOrder}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hPair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hDup : (concatLive levels).Nodup)
    (hmem : oid ∈ liveIds (ordersAt levels price))
    (hfind : findSlot orders oid = some s) :
    toPlainLevels (mark_at levels price s.position_in_level s.remaining) =
      removeAt (toPlainLevels levels) price oid := by
  induction levels with
  | nil => simp [ordersAt, liveIds] at hmem
  | cons head tail ih =>
    have hhead := hInv head mem_cons_self
    have htailI : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hDup)
    have hpair := pairwise_cons.mp hPair
    cases hp : (head.price == price)
    · have hmemt : oid ∈ liveIds (ordersAt tail price) := by
        simpa [ordersAt, hp] using hmem
      have hih := ih htailI hpair.2 hparts.2.1 hmemt
      simp [mark_at, toPlainLevels, toPlain, removeAt, hp, hih]
    · have hprice : head.price = price := by simpa using hp
      have hmemh : oid ∈ liveIds head.orders := by
        simpa [ordersAt, hp] using hmem
      have hn : (liveIds head.orders).Nodup := hparts.1
      have hshape := mark_tombstone_live hhead hmemh hfind
      have hmarked := levelInv_marked hhead hn hmemh hfind
      have hIff := hmarked.2.2.1
      rcases hhead with ⟨_, _, _, hLive⟩
      rcases hLive oid hmemh with ⟨s0, hfind0, _, _, _, _, _, hget0⟩
      have he : some s0 = some s := by rw [← hfind0, hfind]
      injection he with hs0
      have hget : getAt head.orders (s.position_in_level - head.popped_from_front) = some oid := by
        simpa [hs0] using hget0
      have hnz : oid ≠ 0 := (mem_liveIds.mp hmemh).1
      have hlive : liveIds (mark_tombstone head s.position_in_level s.remaining).orders =
          removeId (liveIds head.orders) oid := by
        rw [hshape]
        exact liveIds_setAt_zero hget hnz hn
      have hpr := mark_tombstone_price head s.position_in_level s.remaining
      cases hempty : is_empty (mark_tombstone head s.position_in_level s.remaining)
      · have hsome : removeId (liveIds head.orders) oid ≠ [] := by
          intro hnil
          have ht : is_empty (mark_tombstone head s.position_in_level s.remaining) = true :=
            hIff.mpr (by simpa [hlive] using hnil)
          simp [hempty] at ht
        have hemptyL : (removeId (liveIds head.orders) oid).isEmpty = false := by
          cases ht : (removeId (liveIds head.orders) oid).isEmpty
          · rfl
          · exact absurd ((list_isEmpty_true).mp ht) hsome
        simp [mark_at, toPlainLevels, toPlain, removeAt, hp, hempty, hprice, hemptyL, hlive, hpr]
      · have hnil : removeId (liveIds head.orders) oid = [] := by
          have ht : liveIds (mark_tombstone head s.position_in_level s.remaining).orders = [] :=
            hIff.mp hempty
          simpa [hlive] using ht
        have hemptyL : (removeId (liveIds head.orders) oid).isEmpty = true :=
          (list_isEmpty_true).mpr hnil
        simp [mark_at, toPlainLevels, toPlain, removeAt, hp, hempty, hprice, hemptyL]

theorem concatLive_mark {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid : Nat} {s : QueueOrder}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hPair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hDup : (concatLive levels).Nodup)
    (hmem : oid ∈ liveIds (ordersAt levels price))
    (hfind : findSlot orders oid = some s) :
    concatLive (mark_at levels price s.position_in_level s.remaining) =
      removeId (concatLive levels) oid := by
  induction levels with
  | nil => simp [ordersAt, liveIds] at hmem
  | cons head tail ih =>
    have hhead := hInv head mem_cons_self
    have htailI : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hDup)
    have hpair := pairwise_cons.mp hPair
    cases hp : (head.price == price)
    · have hmemt : oid ∈ liveIds (ordersAt tail price) := by
        simpa [ordersAt, hp] using hmem
      have hnot : oid ∉ liveIds head.orders := by
        intro ha
        exact hparts.2.2 oid ha (mem_ordersAt_concat hmemt)
      have hcut := removeId_append_right (l := liveIds head.orders)
        (r := concatLive tail) hnot
      have hih := ih htailI hpair.2 hparts.2.1 hmemt
      simp [concatLive, mark_at, hp, hcut, hih]
    · have hprice : head.price = price := by simpa using hp
      have hmemh : oid ∈ liveIds head.orders := by
        simpa [ordersAt, hp] using hmem
      have hn : (liveIds head.orders).Nodup := hparts.1
      have hshape := mark_tombstone_live hhead hmemh hfind
      have hmarked := levelInv_marked hhead hn hmemh hfind
      have hIff := hmarked.2.2.1
      have hnottail : oid ∉ concatLive tail := hparts.2.2 oid hmemh
      have hcut := removeId_append_left (l := liveIds head.orders)
        (r := concatLive tail) hnottail
      rcases hhead with ⟨_, _, _, hLive⟩
      rcases hLive oid hmemh with ⟨s0, hfind0, _, _, _, _, _, hget0⟩
      have he : some s0 = some s := by rw [← hfind0, hfind]
      injection he with hs0
      have hget : getAt head.orders (s.position_in_level - head.popped_from_front) = some oid := by
        simpa [hs0] using hget0
      have hnz : oid ≠ 0 := (mem_liveIds.mp hmemh).1
      have hlive : liveIds (mark_tombstone head s.position_in_level s.remaining).orders =
          removeId (liveIds head.orders) oid := by
        rw [hshape]
        exact liveIds_setAt_zero hget hnz hn
      cases hempty : is_empty (mark_tombstone head s.position_in_level s.remaining)
      · simp [concatLive, mark_at, hp, hempty, hcut, hlive]
      · have hnil : removeId (liveIds head.orders) oid = [] := by
          simpa [hlive] using hIff.mp hempty
        simp [concatLive, mark_at, hp, hempty, hcut, hnil]

theorem live_nonempty_mark {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid : Nat} {s : QueueOrder}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hPair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hDup : (concatLive levels).Nodup)
    (hmem : oid ∈ liveIds (ordersAt levels price))
    (hfind : findSlot orders oid = some s)
    (hN : ∀ level ∈ levels, liveIds level.orders ≠ []) :
    ∀ level ∈ mark_at levels price s.position_in_level s.remaining,
      liveIds level.orders ≠ [] := by
  induction levels with
  | nil => simp [ordersAt, liveIds] at hmem
  | cons head tail ih =>
    have hhead := hInv head mem_cons_self
    have htailI : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have htailN : ∀ level ∈ tail, liveIds level.orders ≠ [] :=
      fun level hm => hN level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hDup)
    have hpair := pairwise_cons.mp hPair
    cases hp : (head.price == price)
    · have hmemt : oid ∈ liveIds (ordersAt tail price) := by
        simpa [ordersAt, hp] using hmem
      intro level hm
      simp [mark_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using hN head mem_cons_self
      · exact ih htailI hpair.2 hparts.2.1 hmemt htailN level hm
    · have hmemh : oid ∈ liveIds head.orders := by
        simpa [ordersAt, hp] using hmem
      have hn : (liveIds head.orders).Nodup := hparts.1
      have hshape := mark_tombstone_live hhead hmemh hfind
      have hmarked := levelInv_marked hhead hn hmemh hfind
      have hIff := hmarked.2.2.1
      rcases hhead with ⟨_, _, _, hLive⟩
      rcases hLive oid hmemh with ⟨s0, hfind0, _, _, _, _, _, hget0⟩
      have he : some s0 = some s := by rw [← hfind0, hfind]
      injection he with hs0
      have hget : getAt head.orders (s.position_in_level - head.popped_from_front) = some oid := by
        simpa [hs0] using hget0
      have hnz : oid ≠ 0 := (mem_liveIds.mp hmemh).1
      have hlive : liveIds (mark_tombstone head s.position_in_level s.remaining).orders =
          removeId (liveIds head.orders) oid := by
        rw [hshape]
        exact liveIds_setAt_zero hget hnz hn
      cases hempty : is_empty (mark_tombstone head s.position_in_level s.remaining)
      · intro level hm
        simp [mark_at, hp, hempty] at hm
        rcases hm with heq | hm
        · intro hnil
          have ht : is_empty (mark_tombstone head s.position_in_level s.remaining) = true :=
            hIff.mpr (by simpa [heq, hlive] using hnil)
          simp [hempty] at ht
        · exact htailN level hm
      · intro level hm
        simp [mark_at, hp, hempty] at hm
        exact htailN level hm

theorem live_ordersAt_mark {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid : Nat} {s : QueueOrder} {here : Int}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hPair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hDup : (concatLive levels).Nodup)
    (hmem : oid ∈ liveIds (ordersAt levels price))
    (hfind : findSlot orders oid = some s) :
    liveIds (ordersAt (mark_at levels price s.position_in_level s.remaining) here) =
      if here = price then removeId (liveIds (ordersAt levels price)) oid
      else liveIds (ordersAt levels here) := by
  induction levels with
  | nil => simp [ordersAt, liveIds] at hmem
  | cons head tail ih =>
    have hhead := hInv head mem_cons_self
    have htailI : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hDup)
    have hpair := pairwise_cons.mp hPair
    cases hp : (head.price == price)
    · have hneP : head.price ≠ price := by simpa [beq_iff_eq] using hp
      have hmemt : oid ∈ liveIds (ordersAt tail price) := by
        simpa [ordersAt, hp] using hmem
      have hih := ih htailI hpair.2 hparts.2.1 hmemt
      by_cases hhere : here = price
      · have hph : (head.price == here) = false := by simpa [hhere] using hneP
        simpa [mark_at, ordersAt, hp, hph, hhere] using hih
      · by_cases hh : here = head.price
        · have hb : (head.price == here) = true := by simpa using hh.symm
          simp [mark_at, ordersAt, hp, hb, hhere]
        · have hb : (head.price == here) = false := by simpa using (Ne.symm hh)
          simp [mark_at, ordersAt, hp, hb, hhere, hih]
    · have hprice : head.price = price := by simpa using hp
      have hmemh : oid ∈ liveIds head.orders := by
        simpa [ordersAt, hp] using hmem
      have hn : (liveIds head.orders).Nodup := hparts.1
      have hshape := mark_tombstone_live hhead hmemh hfind
      have hmarked := levelInv_marked hhead hn hmemh hfind
      have hIff := hmarked.2.2.1
      rcases hhead with ⟨_, _, _, hLive⟩
      rcases hLive oid hmemh with ⟨s0, hfind0, _, _, _, _, _, hget0⟩
      have he : some s0 = some s := by rw [← hfind0, hfind]
      injection he with hs0
      have hget : getAt head.orders (s.position_in_level - head.popped_from_front) = some oid := by
        simpa [hs0] using hget0
      have hnz : oid ≠ 0 := (mem_liveIds.mp hmemh).1
      have hlive : liveIds (mark_tombstone head s.position_in_level s.remaining).orders =
          removeId (liveIds head.orders) oid := by
        rw [hshape]
        exact liveIds_setAt_zero hget hnz hn
      have hpr := mark_tombstone_price head s.position_in_level s.remaining
      have htailP : ordersAt tail price = [] := ordersAt_tail_ne hPair hprice
      cases hempty : is_empty (mark_tombstone head s.position_in_level s.remaining)
      · by_cases hhere : here = price
        · have heqP : (mark_tombstone head s.position_in_level s.remaining).price = price :=
            hpr.trans hprice
          simp [mark_at, ordersAt, hp, hempty, heqP, hhere, hlive]
        · have hb : ((mark_tombstone head s.position_in_level s.remaining).price == here) = false := by
            simpa [hpr, hprice] using (Ne.symm hhere)
          have hph : (head.price == here) = false := by simpa [hprice] using (Ne.symm hhere)
          simp [mark_at, ordersAt, hp, hempty, hb, hph, hhere]
      · have hnil : removeId (liveIds head.orders) oid = [] := by
          simpa [hlive] using hIff.mp hempty
        by_cases hhere : here = price
        · simp [mark_at, ordersAt, liveIds, hp, hempty, hhere, htailP, hnil, hprice]
        · have hph : (head.price == here) = false := by simpa [hprice] using (Ne.symm hhere)
          simp [mark_at, ordersAt, hp, hempty, hph, hhere]

theorem active_mark_mem {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid : Nat} {s : QueueOrder}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hPair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hDup : (concatLive levels).Nodup)
    (hmem : oid ∈ liveIds (ordersAt levels price))
    (hfind : findSlot orders oid = some s)
    {j : Nat} {s' : QueueOrder}
    (hj : j ≠ oid)
    (hmemj : j ∈ liveIds (ordersAt levels s'.price)) :
    j ∈ liveIds (ordersAt (mark_at levels price s.position_in_level s.remaining) s'.price) := by
  have hlive := live_ordersAt_mark (here := s'.price) hInv hPair hDup hmem hfind
  by_cases hp : s'.price = price
  · rw [hp] at hlive hmemj ⊢
    have hlive' :
        liveIds (ordersAt (mark_at levels price s.position_in_level s.remaining) price) =
          removeId (liveIds (ordersAt levels price)) oid := by
      simpa using hlive
    have hmemr : j ∈ removeId (liveIds (ordersAt levels price)) oid :=
      (mem_removeId_iff_of_ne hj).mpr hmemj
    rw [hlive']
    exact hmemr
  · have hlive' :
        liveIds (ordersAt (mark_at levels price s.position_in_level s.remaining) s'.price) =
          liveIds (ordersAt levels s'.price) := by
      simpa [hp] using hlive
    simpa [hlive'] using hmemj

theorem holds_cancel {q : QueueBook} {plain : Book} {oid : Nat} (h : holds q plain) :
    holds (applyQ (.cancel oid) q) (applyPlain (.cancel oid) plain) := by
  rcases h with ⟨hAbs, hBook, hLink⟩
  cases hfind : findSlot q.orders oid with
  | none =>
    have hOne := hLink oid
    simp [hfind] at hOne
    cases hfo : findOrder plain.orders oid with
    | none =>
      simp [applyQ, applyPlain, cancel_order, cancelOrder, hfind, hfo]
      exact ⟨hAbs, hBook, hLink⟩
    | some o => simp [hfo] at hOne
  | some s =>
    cases hact : s.active with
    | false =>
      have hOne := hLink oid
      simp [hfind] at hOne
      cases hfo : findOrder plain.orders oid with
      | none => simp [hfo] at hOne
      | some o =>
        simp [hfo] at hOne
        have hinact : o.status.isActive = false := by
          simpa [slotAgrees, hact] using hOne.2.2.2.2
        have hnot : (!o.status.isActive) = true := by simp [hinact]
        have hbf : (s.active == false) = true := by simp [hact]
        simp [applyQ, applyPlain, cancel_order, cancelOrder, hfind, hbf, hfo, hnot]
        exact ⟨hAbs, hBook, hLink⟩
    | true =>
      have hOne := hLink oid
      simp [hfind] at hOne
      cases hfo : findOrder plain.orders oid with
      | none => simp [hfo] at hOne
      | some o =>
        simp [hfo] at hOne
        have hid : o.id = oid := hOne.1
        have hside : s.side = o.side := hOne.2.1
        have hprice : s.price = o.price := hOne.2.2.1
        have hactive : o.status.isActive = true := by
          simpa [slotAgrees, hact] using hOne.2.2.2.2
        rcases hBook with ⟨hBid, hAsk, hBidI, hAskI, hBidN, hAskN, hDup, hKeys, hAct⟩
        rcases hAbs with ⟨hBids, hAsks⟩
        have hmem0 := hAct oid s hfind hact
        let o' : Order := { o with remaining := 0, status := .cancelled }
        cases hs : s.side with
        | buy =>
          have hmemB : oid ∈ liveIds (ordersAt q.bids s.price) := by
            simpa [queueLevels, hs] using hmem0
          have hPair := pairwise_gt_ne hBid
          have hparts := nodup_append_parts hDup
          have hsideB : o.side = .buy := by simpa [hs] using hside.symm
          have hpriceB : o.price = s.price := hprice.symm
          have hbf : (s.active == false) = false := by simp [hact]
          have hnot : (!o.status.isActive) = false := by simp [hactive]
          have hBookQ : applyQ (.cancel oid) q = {
              bids := mark_at q.bids s.price s.position_in_level s.remaining
              asks := q.asks
              orders := upsertSlot q.orders oid (slotOff s) } := by
            simp [applyQ, cancel_order, hfind, hbf, hs, writeSlot, slotOff,
              setQueueLevels, queueLevels]
          have hPlain : applyPlain (.cancel oid) plain = {
              bids := removeAt plain.bids s.price oid
              asks := plain.asks
              orders := upsert plain.orders o'
              nextOrderId := plain.nextOrderId
              nextTradeId := plain.nextTradeId
              nextTimestamp := plain.nextTimestamp } := by
            simp [applyPlain, cancelOrder, hfo, hnot, dequeue, writeOrder, setLevels,
              levelsOf, hsideB, hpriceB, o']
          refine ⟨?_, ?_, ?_⟩
          · rw [hBookQ, hPlain]
            simp [abstractsBook, hAsks]
            simpa [hBids, hpriceB] using
              toPlainLevels_mark hBidI hPair hparts.1 hmemB hfind
          · rw [hBookQ]
            refine ⟨pairwise_mark_at hBid, hAsk, ?_, ?_, ?_, hAskN, ?_, ?_, ?_⟩
            · exact levelsInv_mark hBidI hPair hparts.1 hmemB hfind
            · intro level hm
              exact levelInv_upsert_other (hAskI level hm)
                (not_mem_live_level (hparts.2.2 oid (mem_ordersAt_concat hmemB)) hm)
            · exact live_nonempty_mark hBidI hPair hparts.1 hmemB hfind hBidN
            · simpa [concatLive_mark hBidI hPair hparts.1 hmemB hfind] using
                nodup_remove_append hDup
            · simpa [slotKeys_upsert_existing hfind] using hKeys
            · intro j s' hfind' hactive'
              by_cases hj : j = oid
              · have hslot : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                    some (slotOff s) := by simp [findSlot_upsert]
                have hfind1 : findSlot (upsertSlot q.orders oid (slotOff s)) oid = some s' := by
                  simpa [hj] using hfind'
                have heqs : some s' = some (slotOff s) := by rw [← hfind1, hslot]
                injection heqs with hs'
                simp [hs', slotOff] at hactive'
              · have hfindOld : findSlot q.orders j = some s' := by
                  simpa [findSlot_upsert, hj] using hfind'
                have hmemj := hAct j s' hfindOld hactive'
                cases hss : s'.side with
                | buy =>
                  have hmemjB : j ∈ liveIds (ordersAt q.bids s'.price) := by
                    simpa [queueLevels, hss] using hmemj
                  simpa [queueLevels, hss] using
                    active_mark_mem hBidI hPair hparts.1 hmemB hfind hj hmemjB
                | sell =>
                  simpa [queueLevels, hss] using hmemj
          · intro j
            have hOld := hLink j
            rw [hBookQ, hPlain]
            by_cases hj : j = oid
            · have hfs' : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                  some (slotOff s) := by simp [findSlot_upsert]
              have hfo' : findOrder (upsert plain.orders o') oid = some o' := by
                simp [findOrder_upsert, o', hid]
              simp only [hj, linked, hfs', hfo']
              refine ⟨by simp [o', hid], ?_, ?_, ?_, ?_⟩
              · simpa [slotOff] using hside
              · simpa [slotOff] using hprice
              · simp [slotOff, o']
              · simp [slotOff, o', OrderStatus.isActive]
            · have hfs := findSlot_upsert q.orders oid (slotOff s) j
              have hfo2 := findOrder_upsert plain.orders o' j
              have hid' : j ≠ o'.id := by
                intro heq
                exact hj (by simpa [o', hid] using heq)
              simp [linked, hfs, hfo2, hj, hid'] at hOld ⊢
              exact hOld
        | sell =>
          have hmemA : oid ∈ liveIds (ordersAt q.asks s.price) := by
            simpa [queueLevels, hs] using hmem0
          have hPair := pairwise_lt_ne hAsk
          have hparts := nodup_append_parts hDup
          have hsideS : o.side = .sell := by simpa [hs] using hside.symm
          have hpriceS : o.price = s.price := hprice.symm
          have hbf : (s.active == false) = false := by simp [hact]
          have hnot : (!o.status.isActive) = false := by simp [hactive]
          have hBookQ : applyQ (.cancel oid) q = {
              bids := q.bids
              asks := mark_at q.asks s.price s.position_in_level s.remaining
              orders := upsertSlot q.orders oid (slotOff s) } := by
            simp [applyQ, cancel_order, hfind, hbf, hs, writeSlot, slotOff,
              setQueueLevels, queueLevels]
          have hPlain : applyPlain (.cancel oid) plain = {
              bids := plain.bids
              asks := removeAt plain.asks s.price oid
              orders := upsert plain.orders o'
              nextOrderId := plain.nextOrderId
              nextTradeId := plain.nextTradeId
              nextTimestamp := plain.nextTimestamp } := by
            simp [applyPlain, cancelOrder, hfo, hnot, dequeue, writeOrder, setLevels,
              levelsOf, hsideS, hpriceS, o']
          refine ⟨?_, ?_, ?_⟩
          · rw [hBookQ, hPlain]
            simp [abstractsBook, hBids]
            simpa [hAsks, hpriceS] using
              toPlainLevels_mark hAskI hPair hparts.2.1 hmemA hfind
          · rw [hBookQ]
            refine ⟨hBid, pairwise_mark_at hAsk, ?_, ?_, hBidN, ?_, ?_, ?_, ?_⟩
            · intro level hm
              exact levelInv_upsert_other (hBidI level hm)
                (not_mem_live_level
                  (fun ha => hparts.2.2 oid ha (mem_ordersAt_concat hmemA)) hm)
            · exact levelsInv_mark hAskI hPair hparts.2.1 hmemA hfind
            · exact live_nonempty_mark hAskI hPair hparts.2.1 hmemA hfind hAskN
            · have hcut : concatLive (mark_at q.asks s.price s.position_in_level s.remaining) =
                  removeId (concatLive q.asks) oid :=
                concatLive_mark hAskI hPair hparts.2.1 hmemA hfind
              simpa [hcut] using nodup_append_remove hDup
            · simpa [slotKeys_upsert_existing hfind] using hKeys
            · intro j s' hfind' hactive'
              by_cases hj : j = oid
              · have hslot : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                    some (slotOff s) := by simp [findSlot_upsert]
                have hfind1 : findSlot (upsertSlot q.orders oid (slotOff s)) oid = some s' := by
                  simpa [hj] using hfind'
                have heqs : some s' = some (slotOff s) := by rw [← hfind1, hslot]
                injection heqs with hs'
                simp [hs', slotOff] at hactive'
              · have hfindOld : findSlot q.orders j = some s' := by
                  simpa [findSlot_upsert, hj] using hfind'
                have hmemj := hAct j s' hfindOld hactive'
                cases hss : s'.side with
                | buy =>
                  simpa [queueLevels, hss] using hmemj
                | sell =>
                  have hmemjA : j ∈ liveIds (ordersAt q.asks s'.price) := by
                    simpa [queueLevels, hss] using hmemj
                  simpa [queueLevels, hss] using
                    active_mark_mem hAskI hPair hparts.2.1 hmemA hfind hj hmemjA
          · intro j
            have hOld := hLink j
            rw [hBookQ, hPlain]
            by_cases hj : j = oid
            · have hfs' : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                  some (slotOff s) := by simp [findSlot_upsert]
              have hfo' : findOrder (upsert plain.orders o') oid = some o' := by
                simp [findOrder_upsert, o', hid]
              simp only [hj, linked, hfs', hfo']
              refine ⟨by simp [o', hid], ?_, ?_, ?_, ?_⟩
              · simpa [slotOff] using hside
              · simpa [slotOff] using hprice
              · simp [slotOff, o']
              · simp [slotOff, o', OrderStatus.isActive]
            · have hfs := findSlot_upsert q.orders oid (slotOff s) j
              have hfo2 := findOrder_upsert plain.orders o' j
              have hid' : j ≠ o'.id := by
                intro heq
                exact hj (by simpa [o', hid] using heq)
              simp [linked, hfs, hfo2, hj, hid'] at hOld ⊢
              exact hOld

theorem zeroCount_afterPop (ids : List Nat) :
    zeroCount ids = leadingZeros ids + zeroCount (afterPop ids) := by
  induction ids with
  | nil => simp [zeroCount, leadingZeros, afterPop]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [zeroCount, leadingZeros, afterPop, ih]
      omega
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ => simp [zeroCount, leadingZeros, afterPop, h0]

theorem firstLive_dropLead (ids : List Nat) :
    firstLive (dropLeadZeros ids) = firstLive ids := by
  induction ids with
  | nil => simp [dropLeadZeros, firstLive]
  | cons head tail ih =>
    cases h0 : head == 0 with
    | true =>
      have hz : head = 0 := by simpa using h0
      subst hz
      simp [dropLeadZeros, firstLive, ih]
    | false =>
      cases head with
      | zero => simp at h0
      | succ _ => simp [dropLeadZeros, firstLive, h0]

theorem firstLiveAt_front (levels : List QueueLevel) (price : Int) :
    firstLiveAt (front_at levels price).1 price = firstLiveAt levels price := by
  induction levels with
  | nil => simp [front_at, firstLiveAt, ordersAt, firstLive]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simpa [front_at, firstLiveAt, ordersAt, hp] using ih
    · simp [front_at, firstLiveAt, ordersAt, hp, (front_fields level).1, firstLive_dropLead]

theorem head_live_first (ids : List Nat) : (liveIds ids).head? = firstLive ids := by
  induction ids with
  | nil => simp [liveIds, firstLive]
  | cons head tail ih =>
    cases head with
    | zero => simp [liveIds, firstLive, ih]
    | succ _ => simp [liveIds, firstLive]

theorem frontAt_toPlain (levels : List QueueLevel) (price : Int) :
    frontAt (toPlainLevels levels) price = firstLiveAt levels price := by
  induction levels with
  | nil => simp [frontAt, toPlainLevels, firstLiveAt, ordersAt, firstLive]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simp [frontAt, toPlainLevels, toPlain, firstLiveAt, ordersAt, hp, ih]
    · simp [frontAt, toPlainLevels, toPlain, firstLiveAt, ordersAt, hp, head_live_first]

theorem mem_of_first {ids : List Nat} {oid : Nat} (hf : firstLive ids = some oid) :
    oid ∈ liveIds ids := by
  rw [liveIds_afterPop hf]
  exact mem_cons_self

theorem pop_front_fields (level : QueueLevel) (qty : Nat) :
    (pop_front level qty).1 = {
      price := level.price
      orders := afterPop level.orders
      total_quantity := match firstLive level.orders with
        | none => level.total_quantity
        | some _ => level.total_quantity - qty
      tombstone_count := level.tombstone_count - leadingZeros level.orders
      popped_from_front := level.popped_from_front + popSpan level.orders
    } ∧ (pop_front level qty).2 = firstLive level.orders := by
  simpa [pop_front] using
    popLoop_spec level.orders level.price level.popped_from_front level.tombstone_count
      level.total_quantity qty

theorem remove_live_after {ids : List Nat} {oid : Nat} (hf : firstLive ids = some oid) :
    removeId (liveIds ids) oid = liveIds (afterPop ids) := by
  have hl := liveIds_afterPop hf
  have hnz := firstLive_ne_zero hf
  cases oid with
  | zero => exact absurd rfl hnz
  | succ _ => simp [hl, removeId]

theorem not_mem_after {ids : List Nat} {oid : Nat}
    (hf : firstLive ids = some oid) (hn : (liveIds ids).Nodup) :
    oid ∉ liveIds (afterPop ids) := by
  have hn1 := nodup_cons.mp (by simpa [liveIds_afterPop hf] using hn)
  exact hn1.1

theorem is_empty_pop {level : QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {oid qty : Nat}
    (h : levelInv level orders side) (hf : firstLive level.orders = some oid)
    (hqty : remainOf orders oid = qty) :
    is_empty (pop_front level qty).1 = true ↔ liveIds (afterPop level.orders) = [] := by
  rcases h with ⟨_, hTot, _, hLive⟩
  have hfields := (pop_front_fields level qty).1
  rw [hf] at hfields
  have hsum := liveSum_afterPop (orders := orders) hf
  have hpos : ∀ a, a ∈ liveIds (afterPop level.orders) → 0 < remainOf orders a := by
    intro a ha
    have ha0 : a ∈ liveIds level.orders := by
      rw [liveIds_afterPop hf]
      exact mem_cons_of_mem _ ha
    rcases hLive a ha0 with ⟨sa, hfinda, _, hrema, _⟩
    simpa [remainOf, hfinda] using hrema
  have hzero := liveSum_zero_iff (ids := afterPop level.orders) (orders := orders) hpos
  have hcut : liveSum level.orders orders - qty = liveSum (afterPop level.orders) orders := by
    rw [hsum, hqty, Nat.add_comm]
    exact Nat.add_sub_cancel _ _
  rw [hfields, is_empty, beq_iff_eq, hTot, hcut]
  exact hzero

theorem slot_of_first {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid : Nat}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hf : firstLiveAt levels price = some oid) :
    ∃ s, findSlot orders oid = some s ∧ s.active = true ∧ 0 < s.remaining ∧
      s.side = side ∧ s.price = price := by
  induction levels with
  | nil => simp [firstLiveAt, ordersAt, firstLive] at hf
  | cons head tail ih =>
    have htail : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    cases hp : (head.price == price)
    · exact ih htail (by simpa [firstLiveAt, ordersAt, hp] using hf)
    · have hprice : head.price = price := by simpa using hp
      have hfH : firstLive head.orders = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      rcases (hInv head mem_cons_self).2.2.2 oid (mem_of_first hfH) with
        ⟨s, hfind, hact, hrem, hside, hpr, _⟩
      exact ⟨s, hfind, hact, hrem, hside, hpr.trans hprice⟩

theorem mem_price_pop {levels : List QueueLevel} {price : Int} {qty : Nat}
    {level : QueueLevel} (hm : level ∈ (pop_at levels price qty).1) :
    ∃ orig, orig ∈ levels ∧ orig.price = level.price := by
  induction levels with
  | nil => simp [pop_at] at hm
  | cons head tail ih =>
    cases hp : (head.price == price)
    · simp [pop_at, hp] at hm
      rcases hm with heq | hm
      · exact ⟨head, mem_cons_self, by simpa [heq]⟩
      · rcases ih hm with ⟨orig, ho, heq⟩
        exact ⟨orig, mem_cons_of_mem _ ho, heq⟩
    · cases hpop : pop_front head qty with
      | mk level' got =>
        have hpr : level'.price = head.price := by
          have heql : (pop_front head qty).1 = level' := by simp [hpop]
          simpa using congrArg QueueLevel.price (heql.symm.trans (pop_front_fields head qty).1)
        cases hempty : is_empty level'
        · simp [pop_at, hp, hpop, hempty] at hm
          rcases hm with heq | hm
          · exact ⟨head, mem_cons_self, by simpa [heq] using hpr.symm⟩
          · exact ⟨level, mem_cons_of_mem _ hm, rfl⟩
        · simp [pop_at, hp, hpop, hempty] at hm
          exact ⟨level, mem_cons_of_mem _ hm, rfl⟩

theorem pairwise_pop_at {levels : List QueueLevel} {price : Int} {qty : Nat}
    {R : Int → Int → Prop}
    (h : levels.Pairwise (fun a b => R a.price b.price)) :
    ((pop_at levels price qty).1).Pairwise (fun a b => R a.price b.price) := by
  induction levels with
  | nil => simp [pop_at]
  | cons head tail ih =>
    have hpair := pairwise_cons.mp h
    cases hp : (head.price == price)
    · have hshape : (pop_at (head :: tail) price qty).1 =
          head :: (pop_at tail price qty).1 := by simp [pop_at, hp]
      rw [hshape]
      refine pairwise_cons.mpr ⟨?_, ih hpair.2⟩
      intro level hm
      rcases mem_price_pop hm with ⟨orig, ho, heq⟩
      simpa [heq] using hpair.1 orig ho
    · cases hpop : pop_front head qty with
      | mk level' got =>
        cases hempty : is_empty level'
        · have hpr : level'.price = head.price := by
            have heql : (pop_front head qty).1 = level' := by simp [hpop]
            simpa using congrArg QueueLevel.price (heql.symm.trans (pop_front_fields head qty).1)
          simp [pop_at, hp, hpop, hempty]
          refine ⟨fun level hm => by simpa [hpr] using hpair.1 level hm, hpair.2⟩
        · simpa [pop_at, hp, hpop, hempty] using hpair.2

theorem levelInv_popped {level : QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {oid qty : Nat} {s : QueueOrder}
    (h : levelInv level orders side) (hf : firstLive level.orders = some oid)
    (hfind : findSlot orders oid = some s) (hqty : s.remaining = qty)
    (hn : (liveIds level.orders).Nodup) :
    levelInv (pop_front level qty).1 (upsertSlot orders oid (slotOff s)) side := by
  have hremQty : remainOf orders oid = qty := by simp [remainOf, hfind, hqty]
  have hemptyIff := is_empty_pop h hf hremQty
  have hnot := not_mem_after hf hn
  rcases h with ⟨hTomb, hTot, _, hLive⟩
  have hfields := (pop_front_fields level qty).1
  rw [hf] at hfields
  refine ⟨?_, ?_, ?_, ?_⟩
  · rw [hfields, hTomb, zeroCount_afterPop level.orders]
    exact Nat.add_sub_cancel_left _ _
  · have hsum := liveSum_afterPop (orders := orders) hf
    have hskip := liveSum_upsert_skip (orders := orders)
      (ids := afterPop level.orders) (o := slotOff s) hnot
    have hcut : liveSum level.orders orders - qty =
        liveSum (afterPop level.orders) orders := by
      rw [hsum, hremQty, Nat.add_comm]
      exact Nat.add_sub_cancel _ _
    rw [hfields, hTot, hcut]
    exact hskip.symm
  · simpa [hfields] using hemptyIff
  · intro a ha
    have ha' : a ∈ liveIds (afterPop level.orders) := by simpa [hfields] using ha
    have hnea : a ≠ oid := fun heq => hnot (heq ▸ ha')
    have hnz : a ≠ 0 := (mem_liveIds.mp ha').1
    have ha0 : a ∈ liveIds level.orders := by
      rw [liveIds_afterPop hf]
      exact mem_cons_of_mem _ ha'
    rcases hLive a ha0 with ⟨sa, hfinda, hacta, hrema, hsidea, hpricea, hpopa, hgeta⟩
    have hpast := live_index_after hf hgeta hnz hnea
    have hga := getAt_afterPop hgeta hnz hpast
    have hspan : popSpan level.orders = leadingZeros level.orders + 1 := by
      have hs := popSpan_eq level.orders
      rw [hf] at hs
      exact hs
    have hleSpan : popSpan level.orders ≤
        sa.position_in_level - level.popped_from_front := by
      rw [hspan]
      omega
    have hpopF : (pop_front level qty).1.popped_from_front =
        level.popped_from_front + popSpan level.orders := by
      simp [hfields]
    have hordG : (pop_front level qty).1.orders = afterPop level.orders := by
      simp [hfields]
    have hpriceF : (pop_front level qty).1.price = level.price := by
      simp [hfields]
    refine ⟨sa, ?_, hacta, hrema, hsidea, ?_, ?_, ?_⟩
    · simpa [findSlot_upsert, hnea] using hfinda
    · rw [hpriceF]
      exact hpricea
    · rw [hpopF]
      omega
    · rw [hordG, hpopF]
      have hidx : sa.position_in_level - (level.popped_from_front + popSpan level.orders) =
          (sa.position_in_level - level.popped_from_front) - popSpan level.orders := by
        omega
      simpa [hidx] using hga.2

theorem levelsInv_pop {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid qty : Nat} {s : QueueOrder}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hn : (concatLive levels).Nodup) (hf : firstLiveAt levels price = some oid)
    (hfind : findSlot orders oid = some s) (hqty : s.remaining = qty) :
    ∀ level ∈ (pop_at levels price qty).1,
      levelInv level (upsertSlot orders oid (slotOff s)) side := by
  induction levels with
  | nil => simp [firstLiveAt, ordersAt, firstLive] at hf
  | cons head tail ih =>
    have hhead := hInv head mem_cons_self
    have htail : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hn)
    cases hp : (head.price == price)
    · have hfT : firstLiveAt tail price = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have hmemT : oid ∈ concatLive tail :=
        mem_ordersAt_concat (mem_of_first hfT)
      have hnotH : oid ∉ liveIds head.orders := fun hin => hparts.2.2 oid hin hmemT
      intro level hm
      simp [pop_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using levelInv_upsert_other hhead hnotH
      · exact ih htail hparts.2.1 hfT level hm
    · have hfH : firstLive head.orders = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have honly : oid ∈ liveIds head.orders := mem_of_first hfH
      have hnotTail : ∀ level ∈ tail, oid ∉ liveIds level.orders :=
        fun level hm => not_mem_live_level (hparts.2.2 oid honly) hm
      cases hpop : pop_front head qty with
      | mk level' got =>
        cases hempty : is_empty level'
        · intro level hm
          simp [pop_at, hp, hpop, hempty] at hm
          rcases hm with heq | hm
          · simpa [hpop, heq] using levelInv_popped hhead hfH hfind hqty hparts.1
          · exact levelInv_upsert_other (htail level hm) (hnotTail level hm)
        · intro level hm
          simp [pop_at, hp, hpop, hempty] at hm
          exact levelInv_upsert_other (htail level hm) (hnotTail level hm)

theorem toPlainLevels_pop {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid qty : Nat}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hf : firstLiveAt levels price = some oid) (hqty : remainOf orders oid = qty) :
    toPlainLevels (pop_at levels price qty).1 =
      removeAt (toPlainLevels levels) price oid := by
  induction levels with
  | nil => simp [firstLiveAt, ordersAt, firstLive] at hf
  | cons head tail ih =>
    have htail : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    cases hp : (head.price == price)
    · have hfT : firstLiveAt tail price = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      simp [pop_at, toPlainLevels, removeAt, toPlain, hp, ih htail hfT]
    · have hprice : head.price = price := by simpa using hp
      have hfH : firstLive head.orders = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have hemptyIff := is_empty_pop (hInv head mem_cons_self) hfH hqty
      have hlive := remove_live_after hfH
      cases hpop : pop_front head qty with
      | mk level' got =>
        have hemptyIff' :
            is_empty level' = true ↔ liveIds (afterPop head.orders) = [] := by
          simpa [hpop] using hemptyIff
        have hpr : level'.price = head.price := by
          have heql : (pop_front head qty).1 = level' := by simp [hpop]
          simpa using congrArg QueueLevel.price (heql.symm.trans (pop_front_fields head qty).1)
        have horders : level'.orders = afterPop head.orders := by
          have heql : (pop_front head qty).1 = level' := by simp [hpop]
          simpa using congrArg QueueLevel.orders (heql.symm.trans (pop_front_fields head qty).1)
        cases hempty : is_empty level' with
        | true =>
          have hnil : liveIds (afterPop head.orders) = [] := hemptyIff'.mp hempty
          have honly : liveIds head.orders = [oid] := by
            simpa [hnil] using (liveIds_afterPop hfH)
          have hnz := firstLive_ne_zero hfH
          have hremId : removeId (liveIds head.orders) oid = [] := by
            cases oid with
            | zero => exact absurd rfl hnz
            | succ _ => simp [honly, removeId]
          have hemptyL : (removeId (liveIds head.orders) oid).isEmpty = true := by
            simp [hremId]
          have hshapeP : (pop_at (head :: tail) price qty).1 = tail := by
            simp [pop_at, hp, hpop, hempty]
          have hshapeR : removeAt (toPlainLevels (head :: tail)) price oid =
              toPlainLevels tail := by
            simp [toPlainLevels, removeAt, toPlain, hprice, hremId, hemptyL]
          simp [hshapeP, hshapeR]
        | false =>
          have hneNil : liveIds (afterPop head.orders) ≠ [] := by
            intro hnil
            have htrue := hemptyIff'.mpr hnil
            rw [hempty] at htrue
            cases htrue
          have hemptyB : (liveIds (afterPop head.orders)).isEmpty = false := by
            cases ht : (liveIds (afterPop head.orders)).isEmpty
            · rfl
            · exact absurd (list_isEmpty_true.mp ht) hneNil
          have hshapeP : (pop_at (head :: tail) price qty).1 = level' :: tail := by
            simp [pop_at, hp, hpop, hempty]
          have hplain : toPlain level' =
              { price := head.price, orders := liveIds (afterPop head.orders) } := by
            simp [toPlain, hpr, horders]
          have hshapeR : removeAt (toPlainLevels (head :: tail)) price oid =
              { price := head.price, orders := liveIds (afterPop head.orders) } ::
                toPlainLevels tail := by
            simp [toPlainLevels, removeAt, toPlain, hprice, hlive, hemptyB]
          simpa [hshapeP, toPlainLevels, hplain] using hshapeR.symm

theorem concatLive_pop {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid qty : Nat}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hn : (concatLive levels).Nodup) (hf : firstLiveAt levels price = some oid)
    (hqty : remainOf orders oid = qty) :
    concatLive (pop_at levels price qty).1 = removeId (concatLive levels) oid := by
  induction levels with
  | nil => simp [firstLiveAt, ordersAt, firstLive] at hf
  | cons head tail ih =>
    have htail : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hn)
    cases hp : (head.price == price)
    · have hfT : firstLiveAt tail price = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have hnotH : oid ∉ liveIds head.orders := by
        intro hin
        exact hparts.2.2 oid hin (mem_ordersAt_concat (mem_of_first hfT))
      have hcut := removeId_append_right (l := liveIds head.orders)
        (r := concatLive tail) hnotH
      simp [pop_at, concatLive, hp, ih htail hparts.2.1 hfT, hcut]
    · have hprice : head.price = price := by simpa using hp
      have hfH : firstLive head.orders = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have honly := mem_of_first hfH
      have hnotT : oid ∉ concatLive tail := hparts.2.2 oid honly
      have hcut := removeId_append_left (l := liveIds head.orders)
        (r := concatLive tail) hnotT
      have hemptyIff := is_empty_pop (hInv head mem_cons_self) hfH hqty
      have hlive := remove_live_after hfH
      cases hpop : pop_front head qty with
      | mk level' got =>
        have hemptyIff' :
            is_empty level' = true ↔ liveIds (afterPop head.orders) = [] := by
          simpa [hpop] using hemptyIff
        have horders : level'.orders = afterPop head.orders := by
          have heql : (pop_front head qty).1 = level' := by simp [hpop]
          simpa using congrArg QueueLevel.orders (heql.symm.trans (pop_front_fields head qty).1)
        cases hempty : is_empty level' with
        | true =>
          have hnil : liveIds (afterPop head.orders) = [] := hemptyIff'.mp hempty
          have hremId : removeId (liveIds head.orders) oid = [] := by
            simpa [hnil] using hlive
          simp [pop_at, concatLive, hp, hpop, hempty, hcut, hremId]
        | false =>
          simp [pop_at, concatLive, hp, hpop, hempty, horders, hcut, hlive]

theorem live_nonempty_pop {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid qty : Nat}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hf : firstLiveAt levels price = some oid) (hqty : remainOf orders oid = qty)
    (hN : ∀ level ∈ levels, liveIds level.orders ≠ []) :
    ∀ level ∈ (pop_at levels price qty).1, liveIds level.orders ≠ [] := by
  induction levels with
  | nil => simp [firstLiveAt, ordersAt, firstLive] at hf
  | cons head tail ih =>
    have htailI : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have htailN : ∀ level ∈ tail, liveIds level.orders ≠ [] :=
      fun level hm => hN level (mem_cons_of_mem _ hm)
    cases hp : (head.price == price)
    · have hfT : firstLiveAt tail price = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      intro level hm
      simp [pop_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using hN head mem_cons_self
      · exact ih htailI hfT htailN level hm
    · have hfH : firstLive head.orders = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have hemptyIff := is_empty_pop (hInv head mem_cons_self) hfH hqty
      cases hpop : pop_front head qty with
      | mk level' got =>
        have hemptyIff' :
            is_empty level' = true ↔ liveIds (afterPop head.orders) = [] := by
          simpa [hpop] using hemptyIff
        have horders : level'.orders = afterPop head.orders := by
          have heql : (pop_front head qty).1 = level' := by simp [hpop]
          simpa using congrArg QueueLevel.orders (heql.symm.trans (pop_front_fields head qty).1)
        cases hempty : is_empty level'
        · intro level hm
          simp [pop_at, hp, hpop, hempty] at hm
          rcases hm with heq | hm
          · intro hnil
            have htrue := hemptyIff'.mpr (by simpa [heq, horders] using hnil)
            rw [hempty] at htrue
            cases htrue
          · exact htailN level hm
        · intro level hm
          simp [pop_at, hp, hpop, hempty] at hm
          exact htailN level hm

theorem live_ordersAt_pop {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price here : Int} {oid qty : Nat}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hPair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hf : firstLiveAt levels price = some oid) (hqty : remainOf orders oid = qty) :
    liveIds (ordersAt (pop_at levels price qty).1 here) =
      if here = price then removeId (liveIds (ordersAt levels here)) oid
      else liveIds (ordersAt levels here) := by
  induction levels with
  | nil => simp [firstLiveAt, ordersAt, firstLive] at hf
  | cons head tail ih =>
    have htailI : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hpair := pairwise_cons.mp hPair
    cases hp : (head.price == price)
    · have hfT : firstLiveAt tail price = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have hih := ih htailI hpair.2 hfT
      by_cases hhere : here = price
      · have hph : (head.price == here) = false := by simpa [hhere] using hp
        simp [hhere] at hih
        simp [pop_at, ordersAt, hp, hph, hhere]
        exact hih
      · by_cases hph : head.price = here
        · have hb : (head.price == here) = true := by simpa using hph
          simp [pop_at, ordersAt, liveIds, hp, hb, hhere]
        · have hb : (head.price == here) = false := by simpa using hph
          simp [pop_at, ordersAt, hp, hb, hhere, hih]
    · have hprice : head.price = price := by simpa using hp
      have hfH : firstLive head.orders = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have hemptyIff := is_empty_pop (hInv head mem_cons_self) hfH hqty
      have hlive := remove_live_after hfH
      have htailP : ordersAt tail price = [] := ordersAt_tail_ne hPair hprice
      cases hpop : pop_front head qty with
      | mk level' got =>
        have hemptyIff' :
            is_empty level' = true ↔ liveIds (afterPop head.orders) = [] := by
          simpa [hpop] using hemptyIff
        have hpr : level'.price = head.price := by
          have heql : (pop_front head qty).1 = level' := by simp [hpop]
          simpa using congrArg QueueLevel.price (heql.symm.trans (pop_front_fields head qty).1)
        have horders : level'.orders = afterPop head.orders := by
          have heql : (pop_front head qty).1 = level' := by simp [hpop]
          simpa using congrArg QueueLevel.orders (heql.symm.trans (pop_front_fields head qty).1)
        cases hempty : is_empty level' with
        | true =>
          have hnil : liveIds (afterPop head.orders) = [] := hemptyIff'.mp hempty
          have hremId : removeId (liveIds head.orders) oid = [] := by simpa [hnil] using hlive
          by_cases hhere : here = price
          · simp [pop_at, ordersAt, liveIds, hp, hpop, hempty, hhere, hprice, htailP, hremId]
          · have hph : head.price ≠ here := by simpa [hprice] using Ne.symm hhere
            have hb : (head.price == here) = false := by simpa using hph
            simp [pop_at, ordersAt, hp, hpop, hempty, hb, hhere]
        | false =>
          by_cases hhere : here = price
          · have heqP : level'.price = price := by rw [hpr, hprice]
            simp [pop_at, ordersAt, hp, hpop, hempty, heqP, hhere, hprice, horders, hlive]
          · have hph : level'.price ≠ here := by simpa [hpr, hprice] using Ne.symm hhere
            have hb : (level'.price == here) = false := by simpa using hph
            have hb0 : (head.price == here) = false := by
              have hnh : head.price ≠ here := by
                rw [hprice]
                exact Ne.symm hhere
              simpa using hnh
            simp [pop_at, ordersAt, hp, hpop, hempty, hb, hb0, hhere]

theorem liveSum_dec {ids : List Nat} {orders : List (Nat × QueueOrder)}
    {oid : Nat} {s : QueueOrder} {amount : Nat}
    (hm : oid ∈ liveIds ids) (hn : (liveIds ids).Nodup)
    (hfind : findSlot orders oid = some s) (hle : amount ≤ s.remaining) :
    liveSum ids (upsertSlot orders oid { s with remaining := s.remaining - amount }) =
      liveSum ids orders - amount := by
  induction ids with
  | nil => simp [liveIds] at hm
  | cons head tail ih =>
    cases head with
    | zero =>
      simp [liveSum, liveIds] at hm ⊢
      exact ih hm (by simpa [liveIds] using hn)
    | succ n =>
      have hn1 := nodup_cons.mp (by simpa [liveIds] using hn)
      simp [liveIds] at hm
      rcases hm with heq | hm
      · have hnot : oid ∉ liveIds tail := by simpa [heq] using hn1.1
        have hskip := liveSum_upsert_skip (orders := orders) (ids := tail)
          (o := { s with remaining := s.remaining - amount }) hnot
        have hfind' : findSlot orders (n + 1) = some s := by
          simpa [heq] using hfind
        have hremOld : remainOf orders (n + 1) = s.remaining := by
          simp [remainOf, hfind']
        have hremNew : remainOf (upsertSlot orders oid
            { s with remaining := s.remaining - amount }) (n + 1) =
            s.remaining - amount := by
          rw [remainOf_upsert]
          simp [heq]
        simp [liveSum, hremOld, hremNew, hskip]
        omega
      · have hne : n + 1 ≠ oid := by
          intro contra
          exact hn1.1 (by simpa [contra] using hm)
        have hleTail : amount ≤ liveSum tail orders := by
          have hleR : remainOf orders oid ≤ liveSum tail orders := liveSum_mem_le hm
          have hrem : remainOf orders oid = s.remaining := by simp [remainOf, hfind]
          omega
        have ih1 := ih hm hn1.2
        simp [liveSum, remainOf_upsert, hne, ih1]
        omega

theorem levelInv_decreased {level : QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {oid amount : Nat} {s : QueueOrder}
    (h : levelInv level orders side) (hm : oid ∈ liveIds level.orders)
    (hn : (liveIds level.orders).Nodup) (hfind : findSlot orders oid = some s)
    (hlt : amount < s.remaining) :
    levelInv (decrease_quantity level amount)
      (upsertSlot orders oid { s with remaining := s.remaining - amount }) side := by
  rcases h with ⟨hTomb, hTot, hEmpty, hLive⟩
  rcases hLive oid hm with ⟨s0, hfind0, hact0, hrem0, hside0, hprice0, hpop0, hget0⟩
  have heqs : some s0 = some s := hfind0.symm.trans hfind
  injection heqs with hs0
  have hle : amount ≤ s.remaining := by omega
  have hdec := liveSum_dec hm hn hfind hle
  have hneNil : liveIds level.orders ≠ [] := by
    intro hnil
    simp [hnil] at hm
  have hleSum : s.remaining ≤ level.total_quantity := by
    have hleR : remainOf orders oid ≤ liveSum level.orders orders := liveSum_mem_le hm
    simpa [remainOf, hfind, hTot] using hleR
  have hltTot : amount < level.total_quantity := by omega
  refine ⟨?_, ?_, ?_, ?_⟩
  · simpa [decrease_quantity] using hTomb
  · simpa [decrease_quantity, hTot] using hdec.symm
  · simp [decrease_quantity, is_empty, beq_iff_eq]
    constructor
    · intro hzero
      exfalso
      omega
    · intro hnil
      exact absurd hnil hneNil
  · intro a ha
    by_cases hao : a = oid
    · refine ⟨{ s with remaining := s.remaining - amount }, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
      · simp [findSlot_upsert, hao]
      · simpa [hs0] using hact0
      · show 0 < s.remaining - amount
        omega
      · simpa [hs0] using hside0
      · simpa [decrease_quantity, hs0] using hprice0
      · simpa [decrease_quantity, hs0] using hpop0
      · simpa [decrease_quantity, hao, hs0] using hget0
    · rcases hLive a ha with ⟨sa, hfinda, hacta, hrema, hsidea, hpricea, hpopa, hgeta⟩
      refine ⟨sa, ?_, hacta, hrema, hsidea, ?_, ?_, ?_⟩
      · simpa [findSlot_upsert, hao] using hfinda
      · simpa [decrease_quantity] using hpricea
      · simpa [decrease_quantity] using hpopa
      · simpa [decrease_quantity] using hgeta

theorem levelsInv_decrease {levels : List QueueLevel} {orders : List (Nat × QueueOrder)}
    {side : Side} {price : Int} {oid amount : Nat} {s : QueueOrder}
    (hInv : ∀ level ∈ levels, levelInv level orders side)
    (hn : (concatLive levels).Nodup) (hf : firstLiveAt levels price = some oid)
    (hfind : findSlot orders oid = some s) (hlt : amount < s.remaining) :
    ∀ level ∈ decrease_at levels price amount,
      levelInv level (upsertSlot orders oid { s with remaining := s.remaining - amount })
        side := by
  induction levels with
  | nil => simp [firstLiveAt, ordersAt, firstLive] at hf
  | cons head tail ih =>
    have hhead := hInv head mem_cons_self
    have htail : ∀ level ∈ tail, levelInv level orders side :=
      fun level hm => hInv level (mem_cons_of_mem _ hm)
    have hparts := nodup_append_parts (by simpa [concatLive] using hn)
    cases hp : (head.price == price)
    · have hfT : firstLiveAt tail price = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have hnotH : oid ∉ liveIds head.orders := by
        intro hin
        exact hparts.2.2 oid hin (mem_ordersAt_concat (mem_of_first hfT))
      intro level hm
      simp [decrease_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using
          levelInv_upsert_other hhead hnotH
      · exact ih htail hparts.2.1 hfT level hm
    · have hfH : firstLive head.orders = some oid := by
        simpa [firstLiveAt, ordersAt, hp] using hf
      have honly := mem_of_first hfH
      have hnotTail : ∀ level ∈ tail, oid ∉ liveIds level.orders :=
        fun level hm => not_mem_live_level (hparts.2.2 oid honly) hm
      intro level hm
      simp [decrease_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using levelInv_decreased hhead honly hparts.1 hfind hlt
      · exact levelInv_upsert_other (htail level hm) (hnotTail level hm)

theorem map_price_decrease (levels : List QueueLevel) (price : Int) (amount : Nat) :
    (decrease_at levels price amount).map (·.price) = levels.map (·.price) := by
  induction levels with
  | nil => simp [decrease_at]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simp [decrease_at, hp, ih]
    · simp [decrease_at, hp, decrease_quantity, ih]

theorem pairwise_decrease {levels : List QueueLevel} {price : Int} {amount : Nat}
    {R : Int → Int → Prop}
    (h : levels.Pairwise (fun a b => R a.price b.price)) :
    (decrease_at levels price amount).Pairwise (fun a b => R a.price b.price) := by
  exact pairwise_map.mp (by rw [map_price_decrease]; exact pairwise_map.mpr h)

theorem concatLive_decrease (levels : List QueueLevel) (price : Int) (amount : Nat) :
    concatLive (decrease_at levels price amount) = concatLive levels := by
  induction levels with
  | nil => simp [decrease_at, concatLive]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simp [decrease_at, concatLive, hp, ih]
    · simp [decrease_at, concatLive, decrease_quantity, hp, ih]

theorem toPlainLevels_decrease (levels : List QueueLevel) (price : Int) (amount : Nat) :
    toPlainLevels (decrease_at levels price amount) = toPlainLevels levels := by
  induction levels with
  | nil => simp [decrease_at, toPlainLevels]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · simp [decrease_at, toPlainLevels, hp, ih]
    · simp [decrease_at, toPlainLevels, toPlain, decrease_quantity, hp, ih]

theorem live_nonempty_decrease {levels : List QueueLevel} {price : Int} {amount : Nat}
    (h : ∀ level ∈ levels, liveIds level.orders ≠ []) :
    ∀ level ∈ decrease_at levels price amount, liveIds level.orders ≠ [] := by
  induction levels with
  | nil => simp [decrease_at]
  | cons level rest ih =>
    have htail : ∀ level ∈ rest, liveIds level.orders ≠ [] :=
      fun level' hm => h level' (mem_cons_of_mem _ hm)
    cases hp : (level.price == price)
    · intro level' hm
      simp [decrease_at, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using h level mem_cons_self
      · exact ih htail level' hm
    · intro level' hm
      simp [decrease_at, decrease_quantity, hp] at hm
      rcases hm with heq | hm
      · simpa [heq] using h level mem_cons_self
      · exact htail level' hm

theorem live_ordersAt_decrease (levels : List QueueLevel) (price here : Int) (amount : Nat) :
    liveIds (ordersAt (decrease_at levels price amount) here) =
      liveIds (ordersAt levels here) := by
  induction levels with
  | nil => simp [decrease_at, ordersAt, liveIds]
  | cons level rest ih =>
    cases hp : (level.price == price)
    · cases hh : (level.price == here)
      · simp [decrease_at, ordersAt, hp, hh, ih]
      · simp [decrease_at, ordersAt, hp, hh, ih]
    · have hprice : (decrease_quantity level amount).price = level.price := by
        simp [decrease_quantity]
      cases hh : (level.price == here)
      · simp [decrease_at, ordersAt, hp, hh, hprice, ih]
      · simp [decrease_at, ordersAt, decrease_quantity, hp, hh, hprice]

theorem fill_id (o : Order) (qty : Nat) : (fill o qty).id = o.id := by
  unfold fill
  rfl

theorem fill_side (o : Order) (qty : Nat) : (fill o qty).side = o.side := by
  unfold fill
  rfl

theorem fill_price (o : Order) (qty : Nat) : (fill o qty).price = o.price := by
  unfold fill
  rfl

theorem fill_remaining (o : Order) (qty : Nat) :
    (fill o qty).remaining = o.remaining - qty := by
  unfold fill
  rfl

theorem fill_status (o : Order) (qty : Nat) :
    (fill o qty).status =
      if o.remaining - qty == 0 then .filled else .partiallyFilled := by
  unfold fill
  rfl

theorem holds_pop {q : QueueBook} {plain : Book} {side : Side} {price : Int} {qty : Nat}
    (h : holds q plain) (hallow : allowed q (.popFront side price qty)) :
    holds (applyQ (.popFront side price qty) q)
      (applyPlain (.popFront side price qty) plain) := by
  rcases h with ⟨hAbs, hBook, hLink⟩
  rcases hBook with ⟨hBid, hAsk, hBidI, hAskI, hBidN, hAskN, hDup, hKeys, hAct⟩
  rcases hAbs with ⟨hBids, hAsks⟩
  cases side with
  | buy =>
    cases hfront : firstLiveAt q.bids price with
    | none => simp [allowed, queueLevels, hfront] at hallow
    | some oid =>
      rcases (by simpa [allowed, queueLevels, hfront] using hallow) with ⟨hremQty, _hpos⟩
      rcases slot_of_first hBidI hfront with ⟨s, hfind, hact, _, _, hpriceS⟩
      have hqtyS : s.remaining = qty := by simpa [remainOf, hfind] using hremQty
      have hOne := hLink oid
      simp [hfind] at hOne
      cases hfo : findOrder plain.orders oid with
      | none => simp [hfo] at hOne
      | some o =>
        simp [hfo] at hOne
        have hid : o.id = oid := hOne.1
        have hside : s.side = o.side := hOne.2.1
        have hpriceO : s.price = o.price := hOne.2.2.1
        have hsr : s.remaining = o.remaining := by
          simpa [slotAgrees] using hOne.2.2.2.1
        have hord : o.remaining = qty := hsr.symm.trans hqtyS
        let o' : Order := fill o qty
        have hrem0 : o'.remaining = 0 := by
          simpa [o', fill_remaining, hord, Nat.sub_self]
        have hzero : (o'.remaining == 0) = true := by simp [hrem0]
        have hfrontP : frontAt plain.bids price = some oid := by
          rw [← hBids]
          exact (frontAt_toPlain q.bids price).trans hfront
        have hPair := pairwise_gt_ne hBid
        have hparts := nodup_append_parts hDup
        have hBookQ : applyQ (.popFront .buy price qty) q = {
            bids := (pop_at q.bids price qty).1
            asks := q.asks
            orders := upsertSlot q.orders oid (slotOff s) } := by
          simp [applyQ, pop_front_book, queueLevels, hfront, hfind, writeSlot, slotOff,
            setQueueLevels]
        have hPlain : applyPlain (.popFront .buy price qty) plain = {
            bids := removeAt plain.bids price oid
            asks := plain.asks
            orders := upsert plain.orders o'
            nextOrderId := plain.nextOrderId
            nextTradeId := plain.nextTradeId
            nextTimestamp := plain.nextTimestamp } := by
          simp [applyPlain, hfrontP, hfo, o', fill, hord, hzero, dequeue, writeOrder,
            setLevels, levelsOf]
        refine ⟨?_, ?_, ?_⟩
        · rw [hBookQ, hPlain]
          simp [abstractsBook, hAsks]
          simpa [hBids] using toPlainLevels_pop hBidI hfront hremQty
        · rw [hBookQ]
          refine ⟨pairwise_pop_at hBid, hAsk, ?_, ?_, ?_, hAskN, ?_, ?_, ?_⟩
          · exact levelsInv_pop hBidI hparts.1 hfront hfind hqtyS
          · intro level hm
            exact levelInv_upsert_other (hAskI level hm)
              (not_mem_live_level (hparts.2.2 oid (mem_ordersAt_concat (mem_of_first hfront))) hm)
          · exact live_nonempty_pop hBidI hfront hremQty hBidN
          · simpa [concatLive_pop hBidI hparts.1 hfront hremQty] using nodup_remove_append hDup
          · simpa [slotKeys_upsert_existing hfind] using hKeys
          · intro j s' hfind' hactive'
            by_cases hj : j = oid
            · have hslot : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                  some (slotOff s) := by simp [findSlot_upsert]
              have hfind1 : findSlot (upsertSlot q.orders oid (slotOff s)) oid = some s' := by
                simpa [hj] using hfind'
              have heqs : some s' = some (slotOff s) := by rw [← hfind1, hslot]
              injection heqs with hs'
              simp [hs', slotOff] at hactive'
            · have hfindOld : findSlot q.orders j = some s' := by
                simpa [findSlot_upsert, hj] using hfind'
              have hmemj := hAct j s' hfindOld hactive'
              cases hss : s'.side with
              | buy =>
                have hmemjB : j ∈ liveIds (ordersAt q.bids s'.price) := by
                  simpa [queueLevels, hss] using hmemj
                by_cases hpj : s'.price = price
                · have hlive := live_ordersAt_pop hBidI hPair hfront hremQty (here := s'.price)
                  have hmemr : j ∈ removeId (liveIds (ordersAt q.bids s'.price)) oid :=
                    (mem_removeId_iff_of_ne hj).mpr hmemjB
                  have hliveEq : liveIds (ordersAt (pop_at q.bids price qty).1 s'.price) =
                      removeId (liveIds (ordersAt q.bids s'.price)) oid := by
                    simpa [hpj] using hlive
                  simpa [queueLevels, hss, hliveEq] using hmemr
                · have hlive := live_ordersAt_pop hBidI hPair hfront hremQty (here := s'.price)
                  simp [hpj] at hlive
                  simpa [queueLevels, hss, hlive] using hmemjB
              | sell =>
                simpa [queueLevels, hss] using hmemj
        · intro j
          have hOld := hLink j
          rw [hBookQ, hPlain]
          by_cases hj : j = oid
          · have hfs' : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                some (slotOff s) := by simp [findSlot_upsert]
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hfo' : findOrder (upsert plain.orders o') oid = some o' := by
              simp [findOrder_upsert, hidFill]
            simp only [hj, linked, hfs', hfo']
            refine ⟨hidFill, ?_, ?_, ?_, ?_⟩
            · simpa [slotOff, o', fill_side] using hside
            · simpa [slotOff, o', fill_price] using hpriceO
            · simpa [slotOff, o', fill_remaining, hord, Nat.sub_self]
            · have hst : o'.status = .filled := by
                simpa [o', fill_status, hord, Nat.sub_self]
              simp [slotOff, hst, OrderStatus.isActive]
          · have hfs := findSlot_upsert q.orders oid (slotOff s) j
            have hfo2 := findOrder_upsert plain.orders o' j
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hid' : j ≠ o'.id := by
              intro heq
              exact hj (heq.trans hidFill)
            simp [linked, hfs, hfo2, hj, hid'] at hOld ⊢
            exact hOld
  | sell =>
    cases hfront : firstLiveAt q.asks price with
    | none => simp [allowed, queueLevels, hfront] at hallow
    | some oid =>
      rcases (by simpa [allowed, queueLevels, hfront] using hallow) with ⟨hremQty, _hpos⟩
      rcases slot_of_first hAskI hfront with ⟨s, hfind, hact, _, _, hpriceS⟩
      have hqtyS : s.remaining = qty := by simpa [remainOf, hfind] using hremQty
      have hOne := hLink oid
      simp [hfind] at hOne
      cases hfo : findOrder plain.orders oid with
      | none => simp [hfo] at hOne
      | some o =>
        simp [hfo] at hOne
        have hid : o.id = oid := hOne.1
        have hside : s.side = o.side := hOne.2.1
        have hpriceO : s.price = o.price := hOne.2.2.1
        have hsr : s.remaining = o.remaining := by
          simpa [slotAgrees] using hOne.2.2.2.1
        have hord : o.remaining = qty := hsr.symm.trans hqtyS
        let o' : Order := fill o qty
        have hrem0 : o'.remaining = 0 := by
          simpa [o', fill_remaining, hord, Nat.sub_self]
        have hzero : (o'.remaining == 0) = true := by simp [hrem0]
        have hfrontP : frontAt plain.asks price = some oid := by
          rw [← hAsks]
          exact (frontAt_toPlain q.asks price).trans hfront
        have hPair := pairwise_lt_ne hAsk
        have hparts := nodup_append_parts hDup
        have hBookQ : applyQ (.popFront .sell price qty) q = {
            bids := q.bids
            asks := (pop_at q.asks price qty).1
            orders := upsertSlot q.orders oid (slotOff s) } := by
          simp [applyQ, pop_front_book, queueLevels, hfront, hfind, writeSlot, slotOff,
            setQueueLevels]
        have hPlain : applyPlain (.popFront .sell price qty) plain = {
            bids := plain.bids
            asks := removeAt plain.asks price oid
            orders := upsert plain.orders o'
            nextOrderId := plain.nextOrderId
            nextTradeId := plain.nextTradeId
            nextTimestamp := plain.nextTimestamp } := by
          simp [applyPlain, hfrontP, hfo, o', fill, hord, hzero, dequeue, writeOrder,
            setLevels, levelsOf]
        refine ⟨?_, ?_, ?_⟩
        · rw [hBookQ, hPlain]
          simp [abstractsBook, hBids]
          simpa [hAsks] using toPlainLevels_pop hAskI hfront hremQty
        · rw [hBookQ]
          refine ⟨hBid, pairwise_pop_at hAsk, ?_, ?_, hBidN, ?_, ?_, ?_, ?_⟩
          · intro level hm
            exact levelInv_upsert_other (hBidI level hm)
              (not_mem_live_level
                (fun hin => hparts.2.2 oid hin (mem_ordersAt_concat (mem_of_first hfront))) hm)
          · exact levelsInv_pop hAskI hparts.2.1 hfront hfind hqtyS
          · exact live_nonempty_pop hAskI hfront hremQty hAskN
          · simpa [concatLive_pop hAskI hparts.2.1 hfront hremQty] using
              nodup_append_remove hDup
          · simpa [slotKeys_upsert_existing hfind] using hKeys
          · intro j s' hfind' hactive'
            by_cases hj : j = oid
            · have hslot : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                  some (slotOff s) := by simp [findSlot_upsert]
              have hfind1 : findSlot (upsertSlot q.orders oid (slotOff s)) oid = some s' := by
                simpa [hj] using hfind'
              have heqs : some s' = some (slotOff s) := by rw [← hfind1, hslot]
              injection heqs with hs'
              simp [hs', slotOff] at hactive'
            · have hfindOld : findSlot q.orders j = some s' := by
                simpa [findSlot_upsert, hj] using hfind'
              have hmemj := hAct j s' hfindOld hactive'
              cases hss : s'.side with
              | buy =>
                simpa [queueLevels, hss] using hmemj
              | sell =>
                have hmemjA : j ∈ liveIds (ordersAt q.asks s'.price) := by
                  simpa [queueLevels, hss] using hmemj
                by_cases hpj : s'.price = price
                · have hlive := live_ordersAt_pop hAskI hPair hfront hremQty (here := s'.price)
                  have hmemr : j ∈ removeId (liveIds (ordersAt q.asks s'.price)) oid :=
                    (mem_removeId_iff_of_ne hj).mpr hmemjA
                  have hliveEq : liveIds (ordersAt (pop_at q.asks price qty).1 s'.price) =
                      removeId (liveIds (ordersAt q.asks s'.price)) oid := by
                    simpa [hpj] using hlive
                  simpa [queueLevels, hss, hliveEq] using hmemr
                · have hlive := live_ordersAt_pop hAskI hPair hfront hremQty (here := s'.price)
                  simp [hpj] at hlive
                  simpa [queueLevels, hss, hlive] using hmemjA
        · intro j
          have hOld := hLink j
          rw [hBookQ, hPlain]
          by_cases hj : j = oid
          · have hfs' : findSlot (upsertSlot q.orders oid (slotOff s)) oid =
                some (slotOff s) := by simp [findSlot_upsert]
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hfo' : findOrder (upsert plain.orders o') oid = some o' := by
              simp [findOrder_upsert, hidFill]
            simp only [hj, linked, hfs', hfo']
            refine ⟨hidFill, ?_, ?_, ?_, ?_⟩
            · simpa [slotOff, o', fill_side] using hside
            · simpa [slotOff, o', fill_price] using hpriceO
            · simpa [slotOff, o', fill_remaining, hord, Nat.sub_self]
            · have hst : o'.status = .filled := by
                simpa [o', fill_status, hord, Nat.sub_self]
              simp [slotOff, hst, OrderStatus.isActive]
          · have hfs := findSlot_upsert q.orders oid (slotOff s) j
            have hfo2 := findOrder_upsert plain.orders o' j
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hid' : j ≠ o'.id := by
              intro heq
              exact hj (heq.trans hidFill)
            simp [linked, hfs, hfo2, hj, hid'] at hOld ⊢
            exact hOld

theorem holds_decrease {q : QueueBook} {plain : Book} {side : Side} {price : Int}
    {amount : Nat} (h : holds q plain) (hallow : allowed q (.decrease side price amount)) :
    holds (applyQ (.decrease side price amount) q)
      (applyPlain (.decrease side price amount) plain) := by
  rcases h with ⟨hAbs, hBook, hLink⟩
  rcases hBook with ⟨hBid, hAsk, hBidI, hAskI, hBidN, hAskN, hDup, hKeys, hAct⟩
  rcases hAbs with ⟨hBids, hAsks⟩
  cases side with
  | buy =>
    cases hfront : firstLiveAt q.bids price with
    | none => simp [allowed, queueLevels, hfront] at hallow
    | some oid =>
      rcases (by simpa [allowed, queueLevels, hfront] using hallow) with ⟨hposA, hltA⟩
      rcases slot_of_first hBidI hfront with ⟨s, hfind, hact, hremPos, hsideS, hpriceS⟩
      have hremS : s.remaining = remainOf q.orders oid := by simp [remainOf, hfind]
      have hlt : amount < s.remaining := by simpa [hremS] using hltA
      have hOne := hLink oid
      simp [hfind] at hOne
      cases hfo : findOrder plain.orders oid with
      | none => simp [hfo] at hOne
      | some o =>
        simp [hfo] at hOne
        have hid : o.id = oid := hOne.1
        have hside : s.side = o.side := hOne.2.1
        have hpriceO : s.price = o.price := hOne.2.2.1
        have hsr : s.remaining = o.remaining := by
          simpa [slotAgrees] using hOne.2.2.2.1
        have hord : o.remaining = s.remaining := hsr.symm
        have hactive : o.status.isActive = true := by
          simpa [slotAgrees, hact] using hOne.2.2.2.2
        let o' : Order := fill o amount
        have hfrontP : frontAt plain.bids price = some oid := by
          rw [← hBids]
          exact (frontAt_toPlain q.bids price).trans hfront
        have hPair := pairwise_gt_ne hBid
        have hparts := nodup_append_parts hDup
        have hLiveF : firstLiveAt (front_at q.bids price).1 price = some oid := by
          simpa [hfront] using firstLiveAt_front q.bids price
        have hDupF : (concatLive (front_at q.bids price).1).Nodup := by
          simpa [concatLive_front_at] using hparts.1
        have hBookQ : applyQ (.decrease .buy price amount) q = {
            bids := decrease_at (front_at q.bids price).1 price amount
            asks := q.asks
            orders := upsertSlot q.orders oid { s with remaining := s.remaining - amount } } := by
          simp [applyQ, decrease_book, queueLevels, setQueueLevels, hfront, firstLiveAt_front,
            hfind, writeSlot]
        have hPlain : applyPlain (.decrease .buy price amount) plain = {
            bids := plain.bids
            asks := plain.asks
            orders := upsert plain.orders o'
            nextOrderId := plain.nextOrderId
            nextTradeId := plain.nextTradeId
            nextTimestamp := plain.nextTimestamp } := by
          simp [applyPlain, levelsOf, hfrontP, hfo, o', writeOrder]
        refine ⟨?_, ?_, ?_⟩
        · rw [hBookQ, hPlain]
          simp [abstractsBook, hAsks, hBids, toPlainLevels_front, toPlainLevels_decrease]
        · rw [hBookQ]
          refine ⟨?_, hAsk, ?_, ?_, ?_, hAskN, ?_, ?_, ?_⟩
          · exact pairwise_decrease (pairwise_front_at hBid)
          · exact levelsInv_decrease (levelInv_front_at hBidI) hDupF hLiveF hfind hlt
          · intro level hm
            exact levelInv_upsert_other (hAskI level hm)
              (not_mem_live_level (hparts.2.2 oid (mem_ordersAt_concat (mem_of_first hfront))) hm)
          · exact live_nonempty_decrease (live_nonempty_front_at hBidN)
          · simpa [concatLive_decrease, concatLive_front_at] using hDup
          · simpa [slotKeys_upsert_existing hfind] using hKeys
          · intro j s' hfind' hactive'
            by_cases hj : j = oid
            · have hmem0 : oid ∈ liveIds (ordersAt q.bids price) := mem_of_first hfront
              have hlive : liveIds (ordersAt
                  (decrease_at (front_at q.bids price).1 price amount) price) =
                  liveIds (ordersAt q.bids price) := by
                simpa [live_ordersAt_decrease, live_ordersAt_front] using rfl
              have hpriceJ : s'.price = price := by
                have hslot : findSlot (upsertSlot q.orders oid
                    { s with remaining := s.remaining - amount }) oid =
                    some { s with remaining := s.remaining - amount } := by
                  simp [findSlot_upsert]
                have hfind1 := by simpa [hj] using hfind'
                have heqs : some s' = some { s with remaining := s.remaining - amount } := by
                  rw [← hfind1, hslot]
                injection heqs with hs'
                simpa [hs', hpriceS] using rfl
              have hsideJ : s'.side = s.side := by
                have hslot : findSlot (upsertSlot q.orders oid
                    { s with remaining := s.remaining - amount }) oid =
                    some { s with remaining := s.remaining - amount } := by
                  simp [findSlot_upsert]
                have hfind1 := by simpa [hj] using hfind'
                have heqs : some s' = some { s with remaining := s.remaining - amount } := by
                  rw [← hfind1, hslot]
                injection heqs with hs'
                simp [hs']
              simp [hsideS] at hsideJ
              simpa [queueLevels, hsideJ, hpriceJ, hlive, hj] using hmem0
            · have hfindOld : findSlot q.orders j = some s' := by
                simpa [findSlot_upsert, hj] using hfind'
              have hmemj := hAct j s' hfindOld hactive'
              cases hss : s'.side with
              | buy =>
                have hmemjB : j ∈ liveIds (ordersAt q.bids s'.price) := by
                  simpa [queueLevels, hss] using hmemj
                have hlive : liveIds (ordersAt
                    (decrease_at (front_at q.bids price).1 price amount) s'.price) =
                    liveIds (ordersAt q.bids s'.price) := by
                  simpa [live_ordersAt_decrease, live_ordersAt_front]
                simpa [queueLevels, hss, hlive] using hmemjB
              | sell =>
                simpa [queueLevels, hss] using hmemj
        · intro j
          have hOld := hLink j
          rw [hBookQ, hPlain]
          by_cases hj : j = oid
          · have hfs' : findSlot (upsertSlot q.orders oid
                { s with remaining := s.remaining - amount }) oid =
                some { s with remaining := s.remaining - amount } := by
              simp [findSlot_upsert]
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hfo' : findOrder (upsert plain.orders o') oid = some o' := by
              simp [findOrder_upsert, hidFill]
            simp only [hj, linked, hfs', hfo']
            refine ⟨hidFill, ?_, ?_, ?_, ?_⟩
            · simpa [o', fill_side] using hside
            · simpa [o', fill_price] using hpriceO
            · simpa [o', fill_remaining, hord]
            · have hnz : o.remaining - amount ≠ 0 := by
                have hltO : amount < o.remaining := by simpa [hord] using hlt
                omega
              have hb : (o.remaining - amount == 0) = false := by
                cases hbeq : o.remaining - amount == 0 with
                | false => rfl
                | true =>
                  exact absurd (by simpa [beq_iff_eq] using hbeq) hnz
              have hst : o'.status = .partiallyFilled := by
                simpa [o', fill_status, hb]
              simp [hst, hact, OrderStatus.isActive]
          · have hfs := findSlot_upsert q.orders oid
              { s with remaining := s.remaining - amount } j
            have hfo2 := findOrder_upsert plain.orders o' j
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hid' : j ≠ o'.id := by
              intro heq
              exact hj (heq.trans hidFill)
            simp [linked, hfs, hfo2, hj, hid'] at hOld ⊢
            exact hOld
  | sell =>
    cases hfront : firstLiveAt q.asks price with
    | none => simp [allowed, queueLevels, hfront] at hallow
    | some oid =>
      rcases (by simpa [allowed, queueLevels, hfront] using hallow) with ⟨hposA, hltA⟩
      rcases slot_of_first hAskI hfront with ⟨s, hfind, hact, hremPos, hsideS, hpriceS⟩
      have hremS : s.remaining = remainOf q.orders oid := by simp [remainOf, hfind]
      have hlt : amount < s.remaining := by simpa [hremS] using hltA
      have hOne := hLink oid
      simp [hfind] at hOne
      cases hfo : findOrder plain.orders oid with
      | none => simp [hfo] at hOne
      | some o =>
        simp [hfo] at hOne
        have hid : o.id = oid := hOne.1
        have hside : s.side = o.side := hOne.2.1
        have hpriceO : s.price = o.price := hOne.2.2.1
        have hsr : s.remaining = o.remaining := by
          simpa [slotAgrees] using hOne.2.2.2.1
        have hord : o.remaining = s.remaining := hsr.symm
        have hactive : o.status.isActive = true := by
          simpa [slotAgrees, hact] using hOne.2.2.2.2
        let o' : Order := fill o amount
        have hfrontP : frontAt plain.asks price = some oid := by
          rw [← hAsks]
          exact (frontAt_toPlain q.asks price).trans hfront
        have hPair := pairwise_lt_ne hAsk
        have hparts := nodup_append_parts hDup
        have hLiveF : firstLiveAt (front_at q.asks price).1 price = some oid := by
          simpa [hfront] using firstLiveAt_front q.asks price
        have hDupF : (concatLive (front_at q.asks price).1).Nodup := by
          simpa [concatLive_front_at] using hparts.2.1
        have hBookQ : applyQ (.decrease .sell price amount) q = {
            bids := q.bids
            asks := decrease_at (front_at q.asks price).1 price amount
            orders := upsertSlot q.orders oid { s with remaining := s.remaining - amount } } := by
          simp [applyQ, decrease_book, queueLevels, setQueueLevels, hfront, firstLiveAt_front,
            hfind, writeSlot]
        have hPlain : applyPlain (.decrease .sell price amount) plain = {
            bids := plain.bids
            asks := plain.asks
            orders := upsert plain.orders o'
            nextOrderId := plain.nextOrderId
            nextTradeId := plain.nextTradeId
            nextTimestamp := plain.nextTimestamp } := by
          simp [applyPlain, levelsOf, hfrontP, hfo, o', writeOrder]
        refine ⟨?_, ?_, ?_⟩
        · rw [hBookQ, hPlain]
          simp [abstractsBook, hBids, hAsks, toPlainLevels_front, toPlainLevels_decrease]
        · rw [hBookQ]
          refine ⟨hBid, ?_, ?_, ?_, hBidN, ?_, ?_, ?_, ?_⟩
          · exact pairwise_decrease (pairwise_front_at hAsk)
          · intro level hm
            exact levelInv_upsert_other (hBidI level hm)
              (not_mem_live_level
                (fun hin => hparts.2.2 oid hin (mem_ordersAt_concat (mem_of_first hfront))) hm)
          · exact levelsInv_decrease (levelInv_front_at hAskI) hDupF hLiveF hfind hlt
          · exact live_nonempty_decrease (live_nonempty_front_at hAskN)
          · simpa [concatLive_decrease, concatLive_front_at] using hDup
          · simpa [slotKeys_upsert_existing hfind] using hKeys
          · intro j s' hfind' hactive'
            by_cases hj : j = oid
            · have hmem0 : oid ∈ liveIds (ordersAt q.asks price) := mem_of_first hfront
              have hlive : liveIds (ordersAt
                  (decrease_at (front_at q.asks price).1 price amount) price) =
                  liveIds (ordersAt q.asks price) := by
                simpa [live_ordersAt_decrease, live_ordersAt_front] using rfl
              have hpriceJ : s'.price = price := by
                have hslot : findSlot (upsertSlot q.orders oid
                    { s with remaining := s.remaining - amount }) oid =
                    some { s with remaining := s.remaining - amount } := by
                  simp [findSlot_upsert]
                have hfind1 := by simpa [hj] using hfind'
                have heqs : some s' = some { s with remaining := s.remaining - amount } := by
                  rw [← hfind1, hslot]
                injection heqs with hs'
                simpa [hs', hpriceS] using rfl
              have hsideJ : s'.side = s.side := by
                have hslot : findSlot (upsertSlot q.orders oid
                    { s with remaining := s.remaining - amount }) oid =
                    some { s with remaining := s.remaining - amount } := by
                  simp [findSlot_upsert]
                have hfind1 := by simpa [hj] using hfind'
                have heqs : some s' = some { s with remaining := s.remaining - amount } := by
                  rw [← hfind1, hslot]
                injection heqs with hs'
                simp [hs']
              simp [hsideS] at hsideJ
              simpa [queueLevels, hsideJ, hpriceJ, hlive, hj] using hmem0
            · have hfindOld : findSlot q.orders j = some s' := by
                simpa [findSlot_upsert, hj] using hfind'
              have hmemj := hAct j s' hfindOld hactive'
              cases hss : s'.side with
              | buy =>
                simpa [queueLevels, hss] using hmemj
              | sell =>
                have hmemjA : j ∈ liveIds (ordersAt q.asks s'.price) := by
                  simpa [queueLevels, hss] using hmemj
                have hlive : liveIds (ordersAt
                    (decrease_at (front_at q.asks price).1 price amount) s'.price) =
                    liveIds (ordersAt q.asks s'.price) := by
                  simpa [live_ordersAt_decrease, live_ordersAt_front]
                simpa [queueLevels, hss, hlive] using hmemjA
        · intro j
          have hOld := hLink j
          rw [hBookQ, hPlain]
          by_cases hj : j = oid
          · have hfs' : findSlot (upsertSlot q.orders oid
                { s with remaining := s.remaining - amount }) oid =
                some { s with remaining := s.remaining - amount } := by
              simp [findSlot_upsert]
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hfo' : findOrder (upsert plain.orders o') oid = some o' := by
              simp [findOrder_upsert, hidFill]
            simp only [hj, linked, hfs', hfo']
            refine ⟨hidFill, ?_, ?_, ?_, ?_⟩
            · simpa [o', fill_side] using hside
            · simpa [o', fill_price] using hpriceO
            · simpa [o', fill_remaining, hord]
            · have hnz : o.remaining - amount ≠ 0 := by
                have hltO : amount < o.remaining := by simpa [hord] using hlt
                omega
              have hb : (o.remaining - amount == 0) = false := by
                cases hbeq : o.remaining - amount == 0 with
                | false => rfl
                | true =>
                  exact absurd (by simpa [beq_iff_eq] using hbeq) hnz
              have hst : o'.status = .partiallyFilled := by
                simpa [o', fill_status, hb]
              simp [hst, hact, OrderStatus.isActive]
          · have hfs := findSlot_upsert q.orders oid
              { s with remaining := s.remaining - amount } j
            have hfo2 := findOrder_upsert plain.orders o' j
            have hidFill : o'.id = oid := by simpa [o', fill_id] using hid
            have hid' : j ≠ o'.id := by
              intro heq
              exact hj (heq.trans hidFill)
            simp [linked, hfs, hfo2, hj, hid'] at hOld ⊢
            exact hOld

theorem law10_holds : law10 := by
  refine ⟨holds_empty, ?_⟩
  intro q plain step h hallow
  cases step with
  | insert side price oid qty =>
    rcases hallow with ⟨hid, hqty, hfresh⟩
    exact holds_insert h hid hqty hfresh
  | cancel oid => exact holds_cancel h
  | compact => exact holds_compact h
  | front side price => exact holds_front h
  | popFront side price qty => exact holds_pop h hallow
  | decrease side price amount => exact holds_decrease h hallow

end Nanobook

