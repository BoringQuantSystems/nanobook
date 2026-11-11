/-
Well-formed books.
A queued id is an active order at that level's price.
-/

import Nanobook.Laws

namespace Nanobook

open List

theorem removeId_sublist (ids : List Nat) (id : Nat) : List.Sublist (removeId ids id) ids := by
  induction ids with
  | nil => simp [removeId]
  | cons lvl tail ih =>
    cases hb : (lvl == id)
    · simp [removeId, hb]
      exact ih
    · simp [removeId, hb]

theorem removeId_of_not_mem {ids : List Nat} {id : Nat} (h : id ∉ ids) : removeId ids id = ids := by
  induction ids with
  | nil => simp [removeId]
  | cons lvl tail ih =>
    have hlvl : lvl ≠ id := by
      intro heq
      exact h (heq ▸ List.mem_cons_self)
    have htail : id ∉ tail := by
      intro ht
      exact h (List.mem_cons_of_mem lvl ht)
    simp [removeId, hlvl, ih htail]

theorem removeId_not_mem {ids : List Nat} (h : ids.Nodup) (id : Nat) : id ∉ removeId ids id := by
  induction ids with
  | nil => simp [removeId]
  | cons lvl tail ih =>
    cases hb : (lvl == id)
    · simp [removeId, hb]
      constructor
      · intro heq
        have : (lvl == id) = true := by simp [heq]
        simp [this] at hb
      · exact ih (List.nodup_cons.mp h).2
    · simp [removeId, hb]
      have heq : lvl = id := by simpa using hb
      subst heq
      exact (List.nodup_cons.mp h).1

theorem flat_removeAt_sublist (levels : List Level) (price : Int) (id : Nat) :
    List.Sublist ((removeAt levels price id).flatMap (·.orders)) (levels.flatMap (·.orders)) := by
  induction levels with
  | nil => simp [removeAt]
  | cons lvl tail ih =>
    cases hp : (lvl.price == price)
    · simp [removeAt, hp]
      exact ih
    · cases hempty : (removeId lvl.orders id).isEmpty
      · simp [removeAt, hp, hempty]
        exact removeId_sublist lvl.orders id
      · simp [removeAt, hp, hempty]

theorem priceMap_removeAt_sublist (levels : List Level) (price : Int) (id : Nat) :
    List.Sublist ((removeAt levels price id).map (·.price)) (levels.map (·.price)) := by
  induction levels with
  | nil => simp [removeAt]
  | cons lvl tail ih =>
    cases hp : (lvl.price == price)
    · simp [removeAt, hp]
      exact ih
    · cases hempty : (removeId lvl.orders id).isEmpty
      · simp [removeAt, hp, hempty]
      · simp [removeAt, hp, hempty]

theorem noEmpty_removeAt {levels : List Level} (h : noEmptyLevel levels) (price : Int) (id : Nat) :
    noEmptyLevel (removeAt levels price id) := by
  intro level hl
  induction levels with
  | nil => simp [removeAt] at hl
  | cons lvl tail ih =>
    cases hp : (lvl.price == price)
    · simp [removeAt, hp] at hl
      rcases hl with rfl | hl
      · exact h level List.mem_cons_self
      · exact ih (fun l hm => h l (List.mem_cons_of_mem lvl hm)) hl
    · cases hempty : (removeId lvl.orders id).isEmpty
      · simp [removeAt, hp, hempty] at hl
        rcases hl with rfl | hl
        · simpa [List.isEmpty_iff] using hempty
        · exact h level (List.mem_cons_of_mem lvl hl)
      · simp [removeAt, hp, hempty] at hl
        exact h level (List.mem_cons_of_mem lvl hl)

theorem insertBid_level_price (levels : List Level) (price : Int) (id : Nat) (level : Level)
    (h : level ∈ insertBid levels price id) :
    level.price = price ∨ ∃ old ∈ levels, level.price = old.price := by
  induction levels generalizing level with
  | nil =>
    simp [insertBid] at h
    rcases h with rfl
    exact Or.inl rfl
  | cons lvl tail ih =>
    simp only [insertBid] at h
    split at h
    · rcases List.mem_cons.mp h with rfl | hmem
      · exact Or.inl rfl
      · exact Or.inr ⟨level, hmem, rfl⟩
    · split at h
      · rcases List.mem_cons.mp h with rfl | hmem
        · exact Or.inr ⟨lvl, List.mem_cons_self, rfl⟩
        · exact Or.inr ⟨level, List.mem_cons_of_mem lvl hmem, rfl⟩
      · rcases List.mem_cons.mp h with rfl | hmem
        · exact Or.inr ⟨level, List.mem_cons_self, rfl⟩
        · rcases ih level hmem with hp | ⟨old, hm, hq⟩
          · exact Or.inl hp
          · exact Or.inr ⟨old, List.mem_cons_of_mem lvl hm, hq⟩

theorem insertBid_best (levels : List Level) (price : Int) (id : Nat)
    (h : levels.Pairwise (fun a b => a.price > b.price)) :
    (insertBid levels price id).Pairwise (fun a b => a.price > b.price) := by
  induction levels with
  | nil => simp [insertBid]
  | cons lvl tail ih =>
    have htail : ∀ level ∈ tail, lvl.price > level.price := (List.pairwise_cons.mp h).1
    have hrest : tail.Pairwise (fun a b => a.price > b.price) := (List.pairwise_cons.mp h).2
    simp only [insertBid]
    split
    · rename_i hgt
      refine List.pairwise_cons.mpr ⟨?_, h⟩
      · intro level hl
        rcases List.mem_cons.mp hl with rfl | hl
        · exact hgt
        · exact Int.lt_trans (htail level hl) hgt
    · split
      · refine List.pairwise_cons.mpr ⟨?_, hrest⟩
        · intro level hl
          exact htail level hl
      · rename_i hnot hne
        refine List.pairwise_cons.mpr ⟨?_, ih hrest⟩
        · intro level hl
          rcases insertBid_level_price tail price id level hl with hp | ⟨old, hm, hq⟩
          · have hne' : price ≠ lvl.price := by
              intro heq
              apply hne
              simp [heq]
            have hlt : lvl.price > price := by omega
            simpa [hp] using hlt
          · simpa [hq] using htail old hm

theorem insertAsk_level_price (levels : List Level) (price : Int) (id : Nat) (level : Level)
    (h : level ∈ insertAsk levels price id) :
    level.price = price ∨ ∃ old ∈ levels, level.price = old.price := by
  induction levels generalizing level with
  | nil =>
    simp [insertAsk] at h
    rcases h with rfl
    exact Or.inl rfl
  | cons lvl tail ih =>
    simp only [insertAsk] at h
    split at h
    · rcases mem_cons.mp h with rfl | hmem
      · exact Or.inl rfl
      · exact Or.inr ⟨level, hmem, rfl⟩
    · split at h
      · rcases mem_cons.mp h with rfl | hmem
        · exact Or.inr ⟨lvl, mem_cons_self, rfl⟩
        · exact Or.inr ⟨level, mem_cons_of_mem lvl hmem, rfl⟩
      · rcases mem_cons.mp h with rfl | hmem
        · exact Or.inr ⟨level, mem_cons_self, rfl⟩
        · rcases ih level hmem with hp | ⟨old, hm, hq⟩
          · exact Or.inl hp
          · exact Or.inr ⟨old, mem_cons_of_mem lvl hm, hq⟩

theorem insertAsk_best (levels : List Level) (price : Int) (id : Nat)
    (h : levels.Pairwise (fun a b => a.price < b.price)) :
    (insertAsk levels price id).Pairwise (fun a b => a.price < b.price) := by
  induction levels with
  | nil => simp [insertAsk]
  | cons lvl tail ih =>
    have htail : ∀ level ∈ tail, lvl.price < level.price := (pairwise_cons.mp h).1
    have hrest : tail.Pairwise (fun a b => a.price < b.price) := (pairwise_cons.mp h).2
    simp only [insertAsk]
    split
    · rename_i hlt
      refine pairwise_cons.mpr ⟨?_, h⟩
      · intro level hl
        rcases mem_cons.mp hl with rfl | hl
        · exact hlt
        · exact Int.lt_trans hlt (htail level hl)
    · split
      · refine pairwise_cons.mpr ⟨?_, hrest⟩
        · intro level hl
          exact htail level hl
      · rename_i hnot hne
        refine pairwise_cons.mpr ⟨?_, ih hrest⟩
        · intro level hl
          rcases insertAsk_level_price tail price id level hl with hp | ⟨old, hm, hq⟩
          · have hne' : price ≠ lvl.price := by
              intro heq
              apply hne
              simp [heq]
            have hlt : lvl.price < price := by omega
            simpa [hp] using hlt
          · simpa [hq] using htail old hm

theorem mem_removeId {ids : List Nat} {a id : Nat} (h : a ∈ removeId ids id) : a ∈ ids := by
  induction ids with
  | nil => simp [removeId] at h
  | cons lvl tail ih =>
    cases hb : (lvl == id)
    · simp [removeId, hb] at h
      rcases h with rfl | h
      · exact mem_cons_self
      · exact mem_cons_of_mem lvl (ih h)
    · simp [removeId, hb] at h
      exact mem_cons_of_mem lvl h

theorem mem_insertBid_flat {levels : List Level} {price : Int} {id i : Nat}
    (h : i ∈ (insertBid levels price id).flatMap (·.orders)) :
    i = id ∨ i ∈ levels.flatMap (·.orders) := by
  induction levels with
  | nil =>
    simp [insertBid] at h
    exact Or.inl h
  | cons lvl tail ih =>
    simp only [insertBid] at h
    split at h
    · simp [mem_flatMap] at h
      rcases h with rfl | h
      · exact Or.inl rfl
      · rcases h with h | ⟨a, ha, hi⟩
        · exact Or.inr (mem_flatMap.mpr ⟨lvl, mem_cons_self, h⟩)
        · exact Or.inr (mem_flatMap.mpr ⟨a, mem_cons_of_mem _ ha, hi⟩)
    · split at h
      · simp [mem_flatMap] at h
        rcases h with h | h
        · exact Or.inr (mem_flatMap.mpr ⟨lvl, mem_cons_self, h⟩)
        · rcases h with rfl | ⟨a, ha, hi⟩
          · exact Or.inl rfl
          · exact Or.inr (mem_flatMap.mpr ⟨a, mem_cons_of_mem _ ha, hi⟩)
      · simp [mem_flatMap] at h
        rcases h with h | ⟨a, ha, hi⟩
        · exact Or.inr (mem_flatMap.mpr ⟨lvl, mem_cons_self, h⟩)
        · rcases ih (mem_flatMap.mpr ⟨a, ha, hi⟩) with rfl | h
          · exact Or.inl rfl
          · exact Or.inr (mem_append_right _ h)

theorem mem_insertAsk_flat {levels : List Level} {price : Int} {id i : Nat}
    (h : i ∈ (insertAsk levels price id).flatMap (·.orders)) :
    i = id ∨ i ∈ levels.flatMap (·.orders) := by
  induction levels with
  | nil =>
    simp [insertAsk] at h
    exact Or.inl h
  | cons lvl tail ih =>
    simp only [insertAsk] at h
    split at h
    · simp [mem_flatMap] at h
      rcases h with rfl | h
      · exact Or.inl rfl
      · rcases h with h | ⟨a, ha, hi⟩
        · exact Or.inr (mem_flatMap.mpr ⟨lvl, mem_cons_self, h⟩)
        · exact Or.inr (mem_flatMap.mpr ⟨a, mem_cons_of_mem _ ha, hi⟩)
    · split at h
      · simp [mem_flatMap] at h
        rcases h with h | h
        · exact Or.inr (mem_flatMap.mpr ⟨lvl, mem_cons_self, h⟩)
        · rcases h with rfl | ⟨a, ha, hi⟩
          · exact Or.inl rfl
          · exact Or.inr (mem_flatMap.mpr ⟨a, mem_cons_of_mem _ ha, hi⟩)
      · simp [mem_flatMap] at h
        rcases h with h | ⟨a, ha, hi⟩
        · exact Or.inr (mem_flatMap.mpr ⟨lvl, mem_cons_self, h⟩)
        · rcases ih (mem_flatMap.mpr ⟨a, ha, hi⟩) with rfl | h
          · exact Or.inl rfl
          · exact Or.inr (mem_append_right _ h)

theorem nodup_insertBid (levels : List Level) (price : Int) (id : Nat)
    (h : (levels.flatMap (·.orders)).Nodup) (hnew : id ∉ levels.flatMap (·.orders)) :
    ((insertBid levels price id).flatMap (·.orders)).Nodup := by
  induction levels with
  | nil => simp [insertBid]
  | cons lvl tail ih =>
    have hcons := nodup_append.mp h
    have hnewTail : id ∉ tail.flatMap (·.orders) := by
      intro hm
      exact hnew (mem_append_right _ hm)
    have hnewLvl : id ∉ lvl.orders := by
      intro hm
      exact hnew (mem_append_left _ hm)
    simp only [insertBid]
    split
    · simp
      refine ⟨⟨hnewLvl, ?_⟩, h⟩
      intro x hx hm
      exact hnewTail (mem_flatMap.mpr ⟨x, hx, hm⟩)
    · split
      · refine nodup_append.mpr ⟨?_, hcons.2.1, ?_⟩
        · refine nodup_append.mpr ⟨hcons.1, ?_, ?_⟩
          · simp
          · intro a ha b hb
            simp at hb
            subst hb
            intro heq
            exact hnewLvl (heq ▸ ha)
        · intro a ha b hb
          rcases mem_append.mp ha with ha | ha
          · exact hcons.2.2 a ha b hb
          · simp at ha
            subst ha
            intro heq
            exact hnewTail (heq ▸ hb)
      · refine nodup_append.mpr ⟨hcons.1, ih hcons.2.1 hnewTail, ?_⟩
        intro a ha b hb
        rcases mem_insertBid_flat hb with rfl | hb
        · intro heq
          exact hnewLvl (heq ▸ ha)
        · exact hcons.2.2 a ha b hb

theorem nodup_insertAsk (levels : List Level) (price : Int) (id : Nat)
    (h : (levels.flatMap (·.orders)).Nodup) (hnew : id ∉ levels.flatMap (·.orders)) :
    ((insertAsk levels price id).flatMap (·.orders)).Nodup := by
  induction levels with
  | nil => simp [insertAsk]
  | cons lvl tail ih =>
    have hcons := nodup_append.mp h
    have hnewTail : id ∉ tail.flatMap (·.orders) := by
      intro hm
      exact hnew (mem_append_right _ hm)
    have hnewLvl : id ∉ lvl.orders := by
      intro hm
      exact hnew (mem_append_left _ hm)
    simp only [insertAsk]
    split
    · simp
      refine ⟨⟨hnewLvl, ?_⟩, h⟩
      intro x hx hm
      exact hnewTail (mem_flatMap.mpr ⟨x, hx, hm⟩)
    · split
      · refine nodup_append.mpr ⟨?_, hcons.2.1, ?_⟩
        · refine nodup_append.mpr ⟨hcons.1, ?_, ?_⟩
          · simp
          · intro a ha b hb
            simp at hb
            subst hb
            intro heq
            exact hnewLvl (heq ▸ ha)
        · intro a ha b hb
          rcases mem_append.mp ha with ha | ha
          · exact hcons.2.2 a ha b hb
          · simp at ha
            subst ha
            intro heq
            exact hnewTail (heq ▸ hb)
      · refine nodup_append.mpr ⟨hcons.1, ih hcons.2.1 hnewTail, ?_⟩
        intro a ha b hb
        rcases mem_insertAsk_flat hb with rfl | hb
        · intro heq
          exact hnewLvl (heq ▸ ha)
        · exact hcons.2.2 a ha b hb

theorem noEmpty_insertBid {levels : List Level} (h : noEmptyLevel levels) (price : Int) (id : Nat) :
    noEmptyLevel (insertBid levels price id) := by
  intro level hl
  induction levels with
  | nil =>
    simp [insertBid] at hl
    subst hl
    simp
  | cons lvl tail ih =>
    simp only [insertBid] at hl
    split at hl
    · rcases mem_cons.mp hl with rfl | hl
      · simp
      · exact h level hl
    · split at hl
      · rcases mem_cons.mp hl with rfl | hl
        · simp
        · exact h level (mem_cons_of_mem lvl hl)
      · rcases mem_cons.mp hl with rfl | hl
        · exact h level mem_cons_self
        · exact ih (fun l hm => h l (mem_cons_of_mem lvl hm)) hl

theorem noEmpty_insertAsk {levels : List Level} (h : noEmptyLevel levels) (price : Int) (id : Nat) :
    noEmptyLevel (insertAsk levels price id) := by
  intro level hl
  induction levels with
  | nil =>
    simp [insertAsk] at hl
    subst hl
    simp
  | cons lvl tail ih =>
    simp only [insertAsk] at hl
    split at hl
    · rcases mem_cons.mp hl with rfl | hl
      · simp
      · exact h level hl
    · split at hl
      · rcases mem_cons.mp hl with rfl | hl
        · simp
        · exact h level (mem_cons_of_mem lvl hl)
      · rcases mem_cons.mp hl with rfl | hl
        · exact h level mem_cons_self
        · exact ih (fun l hm => h l (mem_cons_of_mem lvl hm)) hl

def keysMatch (orders : List (Nat × Order)) : Prop :=
  ∀ p ∈ orders, p.1 = p.2.id

theorem keysMatch_upsert (orders : List (Nat × Order)) (o : Order) (h : keysMatch orders) :
    keysMatch (upsert orders o) := by
  intro p hp
  induction orders with
  | nil =>
    simp [upsert] at hp
    rcases hp with rfl
    rfl
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    by_cases hi : i = o.id
    · simp [upsert, hi] at hp
      rcases hp with rfl | hp
      · rfl
      · exact h _ (mem_cons_of_mem _ hp)
    · simp [upsert, hi] at hp
      rcases hp with rfl | hp
      · exact h _ mem_cons_self
      · exact ih (fun q hq => h _ (mem_cons_of_mem _ hq)) hp

theorem below_upsert {orders : List (Nat × Order)} {o : Order} {n : Nat}
    (h : ∀ p ∈ orders, p.1 < n) (ho : o.id < n) :
    ∀ p ∈ upsert orders o, p.1 < n := by
  intro p hp
  induction orders with
  | nil =>
    simp [upsert] at hp
    rcases hp with rfl
    exact ho
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    by_cases hi : i = o.id
    · simp [upsert, hi] at hp
      rcases hp with rfl | hp
      · exact ho
      · exact h _ (mem_cons_of_mem _ hp)
    · simp [upsert, hi] at hp
      rcases hp with rfl | hp
      · exact h _ mem_cons_self
      · exact ih (fun q hq => h _ (mem_cons_of_mem _ hq)) hp

theorem findOrder_upsert (orders : List (Nat × Order)) (o : Order) (id : Nat) :
    findOrder (upsert orders o) id = if id = o.id then some o else findOrder orders id := by
  induction orders with
  | nil =>
    by_cases h : id = o.id
    · simp [findOrder, upsert, h]
    · simp [findOrder, upsert, Ne.symm h]
      exact h
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    by_cases hi : i = o.id
    · subst hi
      by_cases hid : id = o.id
      · subst hid
        simp [upsert, findOrder]
      · simp [upsert, findOrder, Ne.symm hid]
        intro contra
        exact absurd contra hid
    · by_cases hid : id = o.id
      · subst hid
        simp [upsert, findOrder, hi, ih]
      · simp [upsert, findOrder, hi, ih]
        by_cases hii : i = id
        · simp [hii, hid]
        · simp [hii, hid]

theorem findOrder_id {orders : List (Nat × Order)} {id : Nat} {o : Order}
    (hkeys : keysMatch orders) (h : findOrder orders id = some o) : o.id = id := by
  induction orders with
  | nil => simp [findOrder] at h
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    cases hb : (i == id)
    · simp [findOrder, hb] at h
      exact ih (fun p hp => hkeys p (mem_cons_of_mem _ hp)) h
    · simp [findOrder, hb] at h
      subst h
      have heq : i = id := by simpa using hb
      have hk := hkeys (i, prev) mem_cons_self
      simpa [heq] using hk.symm

theorem pairwise_price_removeAt {levels : List Level} {price : Int} {id : Nat}
    {R : Int → Int → Prop} (h : levels.Pairwise (fun a b => R a.price b.price)) :
    (removeAt levels price id).Pairwise (fun a b => R a.price b.price) := by
  have hm : (levels.map (·.price)).Pairwise R := pairwise_map.mpr h
  have hm' : ((removeAt levels price id).map (·.price)).Pairwise R :=
    hm.sublist (priceMap_removeAt_sublist levels price id)
  exact pairwise_map.mp hm'

theorem queuedCount_remove_head (price : Int) (id : Nat) (restOrders : List Nat)
    (tail : List Level) :
    queuedCount (removeAt ({ price := price, orders := id :: restOrders } :: tail) price id) <
      queuedCount ({ price := price, orders := id :: restOrders } :: tail) := by
  simp [removeAt, removeId]
  by_cases hempty : restOrders = []
  · simp [hempty, queuedCount]
  · simp [hempty, queuedCount]

theorem findOrder_mem {orders : List (Nat × Order)} {id : Nat} {o : Order}
    (hkeys : keysMatch orders) (h : findOrder orders id = some o) : (id, o) ∈ orders := by
  induction orders with
  | nil => simp [findOrder] at h
  | cons entry tail ih =>
    rcases entry with ⟨i, prev⟩
    cases hb : (i == id)
    · simp [findOrder, hb] at h
      exact mem_cons_of_mem _ (ih (fun p hp => hkeys p (mem_cons_of_mem _ hp)) h)
    · simp [findOrder, hb] at h
      subst h
      have heq : i = id := by simpa using hb
      subst heq
      exact mem_cons_self

theorem removeAt_prefix (pre : List Level) (level : Level) (post : List Level) (id : Nat)
    (hskip : ∀ l ∈ pre, (l.price == level.price) = false) :
    removeAt (pre ++ level :: post) level.price id =
      pre ++ (if (removeId level.orders id).isEmpty = true then post
        else { level with orders := removeId level.orders id } :: post) := by
  induction pre with
  | nil => simp [removeAt]
  | cons lvl tail ih =>
    have hbeq : (lvl.price == level.price) = false := hskip lvl mem_cons_self
    simp [removeAt, hbeq, ih (fun l hl => hskip l (mem_cons_of_mem _ hl))]

theorem best_price_ne {side : Side} {levels : List Level} (h : bestFirst side levels) :
    levels.Pairwise (fun a b => a.price ≠ b.price) := by
  cases side with
  | buy =>
    apply Pairwise.imp _ h
    intro a b hab heq
    omega
  | sell =>
    apply Pairwise.imp _ h
    intro a b hab heq
    omega

structure BookInv (book : Book) : Prop where
  bidOrder : bestFirst .buy book.bids
  askOrder : bestFirst .sell book.asks
  bidFull : noEmptyLevel book.bids
  askFull : noEmptyLevel book.asks
  agreeBid : queuedActive book .buy
  agreeAsk : queuedActive book .sell
  keys : keysMatch book.orders
  nodup : (queueIds book).Nodup
  below : ∀ p ∈ book.orders, p.1 < book.nextOrderId

theorem bookInv_empty : BookInv Book.empty where
  bidOrder := by simp [Book.empty, bestFirst]
  askOrder := by simp [Book.empty, bestFirst]
  bidFull := by simp [Book.empty, noEmptyLevel]
  askFull := by simp [Book.empty, noEmptyLevel]
  agreeBid := by simp [Book.empty, queuedActive, levelsOf]
  agreeAsk := by simp [Book.empty, queuedActive, levelsOf]
  keys := by simp [Book.empty, keysMatch]
  nodup := by simp [Book.empty, queueIds]
  below := by simp [Book.empty]

theorem removeAt_of_absent (levels : List Level) (price : Int) (id : Nat)
    (hfull : noEmptyLevel levels) (hmiss : id ∉ levels.flatMap (·.orders)) :
    removeAt levels price id = levels := by
  induction levels with
  | nil => simp [removeAt]
  | cons lvl tail ih =>
    have htail : id ∉ tail.flatMap (·.orders) := by
      intro hm
      exact hmiss (mem_append_right _ hm)
    have hlvl : id ∉ lvl.orders := by
      intro hm
      exact hmiss (mem_append_left _ hm)
    cases hp : (lvl.price == price)
    · simp [removeAt, hp, ih (fun l hm => hfull l (mem_cons_of_mem _ hm)) htail]
    · have heq := removeId_of_not_mem hlvl
      have hne : lvl.orders ≠ [] := hfull lvl mem_cons_self
      have hempty : lvl.orders.isEmpty = false := by simpa [isEmpty_iff] using hne
      simp [removeAt, hp, heq, hempty]

theorem not_mem_remove_level {levels : List Level} {id : Nat} {level : Level}
    (hpair : levels.Pairwise (fun a b => a.price ≠ b.price))
    (hnd : (levels.flatMap (·.orders)).Nodup)
    (hl : level ∈ levels) (hid : id ∈ level.orders) :
    id ∉ (removeAt levels level.price id).flatMap (·.orders) := by
  rcases append_of_mem hl with ⟨pre, post, rfl⟩
  have hskip : ∀ l ∈ pre, (l.price == level.price) = false := by
    intro l hlm
    have hp := (pairwise_append.mp hpair).2.2 l hlm level mem_cons_self
    simpa using hp
  have hnd' : ((pre.flatMap (·.orders)) ++ (level.orders ++ post.flatMap (·.orders))).Nodup := by
    simpa [flatMap_append] using hnd
  have hparts := nodup_append.mp hnd'
  have hlevel := nodup_append.mp hparts.2.1
  have hidpre : id ∉ pre.flatMap (·.orders) := by
    intro hm
    exact hparts.2.2 id hm id (mem_append_left _ hid) rfl
  have hidpost : id ∉ post.flatMap (·.orders) := by
    intro hm
    exact hlevel.2.2 id hid id hm rfl
  have hidrem : id ∉ removeId level.orders id := removeId_not_mem hlevel.1 id
  rw [removeAt_prefix pre level post id hskip]
  cases hempty : (removeId level.orders id).isEmpty
  · simp [flatMap_append]
    refine ⟨?_, hidrem, ?_⟩
    · intro x hx hm
      exact hidpre (mem_flatMap.mpr ⟨x, hx, hm⟩)
    · intro x hx hm
      exact hidpost (mem_flatMap.mpr ⟨x, hx, hm⟩)
  · simp [flatMap_append]
    refine ⟨?_, ?_⟩
    · intro x hx hm
      exact hidpre (mem_flatMap.mpr ⟨x, hx, hm⟩)
    · intro x hx hm
      exact hidpost (mem_flatMap.mpr ⟨x, hx, hm⟩)

def queueLoad (book : Book) : Nat :=
  queuedCount book.bids + queuedCount book.asks + levelCount book.bids + levelCount book.asks

theorem queuedCount_flat (levels : List Level) :
    queuedCount levels = (levels.flatMap (·.orders)).length := by
  induction levels with
  | nil => simp [queuedCount]
  | cons lvl tail ih => simp [queuedCount, ih]

theorem levelCount_length (levels : List Level) : levelCount levels = levels.length := by
  induction levels with
  | nil => simp [levelCount]
  | cons _ tail ih => simp [levelCount, ih]

theorem length_sublist {α : Type} {a b : List α} (h : Sublist a b) : a.length ≤ b.length := by
  induction h with
  | slnil => simp
  | cons _ _ ih => exact Nat.le_trans ih (Nat.le_succ _)
  | cons_cons _ _ ih => exact Nat.succ_le_succ ih

theorem queuedCount_removeAt_le (levels : List Level) (price : Int) (id : Nat) :
    queuedCount (removeAt levels price id) ≤ queuedCount levels := by
  simpa [queuedCount_flat] using length_sublist (flat_removeAt_sublist levels price id)

theorem levelCount_removeAt_le (levels : List Level) (price : Int) (id : Nat) :
    levelCount (removeAt levels price id) ≤ levelCount levels := by
  simpa [levelCount_length, length_map] using length_sublist (priceMap_removeAt_sublist levels price id)

theorem queueLoad_dequeue_le (book : Book) (side : Side) (price : Int) (id : Nat) :
    queueLoad (dequeue book side price id) ≤ queueLoad book := by
  cases side with
  | buy =>
    simp [queueLoad, dequeue, setLevels, levelsOf]
    have hq := queuedCount_removeAt_le book.bids price id
    have hl := levelCount_removeAt_le book.bids price id
    omega
  | sell =>
    simp [queueLoad, dequeue, setLevels, levelsOf]
    have hq := queuedCount_removeAt_le book.asks price id
    have hl := levelCount_removeAt_le book.asks price id
    omega

theorem queueLoad_write (book : Book) (o : Order) : queueLoad (writeOrder book o) = queueLoad book := by
  simp [queueLoad, writeOrder]

theorem queueLoad_emit (book : Book) (price : Int) (qty aggressor passive : Nat) (side : Side) :
    queueLoad (emitTrade book price qty aggressor passive side).1 = queueLoad book := by
  simp [queueLoad, emitTrade]

theorem queueLoad_dequeue_front {book : Book} {side : Side} {px : Int} {id : Nat}
    {rest : List Nat} {tail : List Level}
    (h : levelsOf book side = { price := px, orders := id :: rest } :: tail) :
    queueLoad (dequeue book side px id) < queueLoad book := by
  cases side with
  | buy =>
    have hbid : book.bids = { price := px, orders := id :: rest } :: tail := by
      simpa [levelsOf] using h
    simp [queueLoad, dequeue, setLevels, levelsOf, hbid]
    have hq := queuedCount_remove_head px id rest tail
    have hl := levelCount_removeAt_le ({ price := px, orders := id :: rest } :: tail) px id
    omega
  | sell =>
    have hask : book.asks = { price := px, orders := id :: rest } :: tail := by
      simpa [levelsOf] using h
    simp [queueLoad, dequeue, setLevels, levelsOf, hask]
    have hq := queuedCount_remove_head px id rest tail
    have hl := levelCount_removeAt_le ({ price := px, orders := id :: rest } :: tail) px id
    omega

theorem mem_of_removeAt {levels : List Level} {price : Int} {id : Nat} {level : Level} {i : Nat}
    (hl : level ∈ removeAt levels price id) (hi : i ∈ level.orders) :
    ∃ old ∈ levels, old.price = level.price ∧ i ∈ old.orders := by
  induction levels generalizing level i with
  | nil => simp [removeAt] at hl
  | cons lvl tail ih =>
    cases hp : (lvl.price == price)
    · simp [removeAt, hp] at hl
      rcases hl with rfl | hl
      · exact ⟨level, mem_cons_self, rfl, hi⟩
      · rcases ih hl hi with ⟨old, hm, hq, ho⟩
        exact ⟨old, mem_cons_of_mem _ hm, hq, ho⟩
    · cases hempty : (removeId lvl.orders id).isEmpty
      · simp [removeAt, hp, hempty] at hl
        rcases hl with rfl | hl
        · exact ⟨lvl, mem_cons_self, rfl, mem_removeId hi⟩
        · exact ⟨level, mem_cons_of_mem _ hl, rfl, hi⟩
      · simp [removeAt, hp, hempty] at hl
        exact ⟨level, mem_cons_of_mem _ hl, rfl, hi⟩

theorem side_flat_nodup (h : BookInv book) (side : Side) :
    ((levelsOf book side).flatMap (·.orders)).Nodup := by
  cases side with
  | buy => exact (nodup_append.mp (by simpa [queueIds, levelsOf] using h.nodup)).1
  | sell => exact (nodup_append.mp (by simpa [queueIds, levelsOf] using h.nodup)).2.1

theorem side_best (h : BookInv book) (side : Side) : bestFirst side (levelsOf book side) := by
  cases side with
  | buy => simpa [levelsOf, bestFirst] using h.bidOrder
  | sell => simpa [levelsOf, bestFirst] using h.askOrder

theorem side_full (h : BookInv book) (side : Side) : noEmptyLevel (levelsOf book side) := by
  cases side with
  | buy => simpa [levelsOf] using h.bidFull
  | sell => simpa [levelsOf] using h.askFull

theorem side_agree (h : BookInv book) (side : Side) : queuedActive book side := by
  cases side with
  | buy => exact h.agreeBid
  | sell => exact h.agreeAsk

theorem agree_replace (h : BookInv book) {o o' : Order} {id : Nat} {side : Side} {px : Int}
    (hfind : findOrder book.orders id = some o) (hid : o'.id = id)
    (hside : o.side = side) (hpx : o.price = px) :
    queuedActive (dequeue (writeOrder book o') side px id) side := by
  intro level hl i hi
  have hlev : levelsOf (dequeue (writeOrder book o') side px id) side =
      removeAt (levelsOf book side) px id := by
    cases side <;> simp [dequeue, writeOrder, setLevels, levelsOf]
  have hl' : level ∈ removeAt (levelsOf book side) px id := by simpa [hlev] using hl
  have horders : (dequeue (writeOrder book o') side px id).orders = upsert book.orders o' := by
    cases side <;> simp [dequeue, writeOrder, setLevels]
  by_cases hmem : id ∈ (levelsOf book side).flatMap (·.orders)
  · rcases mem_flatMap.mp hmem with ⟨lvl0, hl0, hid0⟩
    rcases side_agree h side lvl0 hl0 id hid0 with ⟨o2, hf, hside2, hpx2, _, _⟩
    have ho2 : o2 = o := by
      rw [hf] at hfind
      cases hfind
      rfl
    have hgone := not_mem_remove_level (best_price_ne (side_best h side))
      (side_flat_nodup h side) hl0 hid0
    have hpx0 : lvl0.price = px := by simpa [ho2, hpx] using hpx2.symm
    have hine : i ≠ id := by
      intro heq
      apply hgone
      exact mem_flatMap.mpr ⟨level, by simpa [hpx0] using hl', heq ▸ hi⟩
    have hne : i ≠ o'.id := by simpa [hid] using hine
    rcases mem_of_removeAt hl' hi with ⟨old, hold, hprice, hiold⟩
    rcases side_agree h side old hold i hiold with ⟨oq, hq, hsq, hpq, haq, hrq⟩
    refine ⟨oq, ?_, hsq, by simpa [hprice] using hpq, haq, hrq⟩
    · rw [horders, findOrder_upsert, ite_eq_right hne]
      exact hq
  · have heqLv := removeAt_of_absent (levelsOf book side) px id (side_full h side) hmem
    have hl0 : level ∈ levelsOf book side := by simpa [heqLv] using hl'
    have hine : i ≠ id := by
      intro heqi
      exact hmem (mem_flatMap.mpr ⟨level, hl0, heqi ▸ hi⟩)
    have hne : i ≠ o'.id := by simpa [hid] using hine
    rcases side_agree h side level hl0 i hi with ⟨oq, hq, hsq, hpq, haq, hrq⟩
    refine ⟨oq, ?_, hsq, hpq, haq, hrq⟩
    · rw [horders, findOrder_upsert, ite_eq_right hne]
      exact hq

theorem agree_replace_opp (h : BookInv book) {o o' : Order} {id : Nat} {side : Side} {px : Int}
    (hfind : findOrder book.orders id = some o) (hid : o'.id = id)
    (hside : o.side = side) (_hpx : o.price = px) :
    queuedActive (dequeue (writeOrder book o') side px id) side.opposite := by
  intro level hl i hi
  have hopp : levelsOf (dequeue (writeOrder book o') side px id) side.opposite =
      levelsOf book side.opposite := by
    cases side <;> simp [dequeue, writeOrder, setLevels, levelsOf, Side.opposite]
  have hl0 : level ∈ levelsOf book side.opposite := by simpa [hopp] using hl
  have horders : (dequeue (writeOrder book o') side px id).orders = upsert book.orders o' := by
    cases side <;> simp [dequeue, writeOrder, setLevels]
  have hine : i ≠ id := by
    intro heq
    rcases side_agree h side.opposite level hl0 id (heq ▸ hi) with ⟨oq, hq, hsq, _, _, _⟩
    have hoq : oq = o := by
      rw [hq] at hfind
      cases hfind
      rfl
    cases side <;> simp [Side.opposite, hside, hoq] at hsq
  have hne : i ≠ o'.id := by simpa [hid] using hine
  rcases side_agree h side.opposite level hl0 i hi with ⟨oq, hq, hsq, hpq, haq, hrq⟩
  refine ⟨oq, ?_, hsq, hpq, haq, hrq⟩
  · rw [horders, findOrder_upsert, ite_eq_right hne]
    exact hq

theorem bookInv_replace_out (h : BookInv book) {o o' : Order} {id : Nat} {side : Side} {px : Int}
    (hfind : findOrder book.orders id = some o) (hid : o'.id = id)
    (hside : o.side = side) (hpx : o.price = px) :
    BookInv (dequeue (writeOrder book o') side px id) := by
  let book2 := dequeue (writeOrder book o') side px id
  have horders : book2.orders = upsert book.orders o' := by
    cases side <;> simp [book2, dequeue, writeOrder, setLevels]
  have hnext : book2.nextOrderId = book.nextOrderId := by
    cases side <;> simp [book2, dequeue, writeOrder, setLevels]
  have hsame : levelsOf book2 side = removeAt (levelsOf book side) px id := by
    cases side <;> simp [book2, dequeue, writeOrder, setLevels, levelsOf]
  have hopp : levelsOf book2 side.opposite = levelsOf book side.opposite := by
    cases side <;> simp [book2, dequeue, writeOrder, setLevels, levelsOf, Side.opposite]
  have hkeys : keysMatch book2.orders := by
    simpa [horders] using keysMatch_upsert book.orders o' h.keys
  have hbelow : ∀ p ∈ book2.orders, p.1 < book2.nextOrderId := by
    intro p hp
    have hlt : p.1 < book.nextOrderId := by
      have hmem : (id, o) ∈ book.orders := findOrder_mem h.keys hfind
      have hidlt : id < book.nextOrderId := h.below (id, o) hmem
      have ho' : o'.id < book.nextOrderId := by simpa [hid] using hidlt
      exact below_upsert h.below ho' p (by simpa [horders] using hp)
    simpa [hnext] using hlt
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, hkeys, ?_, hbelow⟩
  · cases side with
    | buy =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf, bestFirst] using
        pairwise_price_removeAt (R := fun a b => a > b) h.bidOrder
    | sell =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf, bestFirst] using h.bidOrder
  · cases side with
    | buy =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf, bestFirst] using h.askOrder
    | sell =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf, bestFirst] using
        pairwise_price_removeAt (R := fun a b => a < b) h.askOrder
  · cases side with
    | buy =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf] using noEmpty_removeAt h.bidFull px id
    | sell =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf] using h.bidFull
  · cases side with
    | buy =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf] using h.askFull
    | sell =>
      simpa [book2, dequeue, writeOrder, setLevels, levelsOf] using noEmpty_removeAt h.askFull px id
  · cases side with
    | buy => simpa [book2] using agree_replace h hfind hid hside hpx
    | sell => simpa [book2, Side.opposite] using agree_replace_opp h hfind hid hside hpx
  · cases side with
    | buy => simpa [book2, Side.opposite] using agree_replace_opp h hfind hid hside hpx
    | sell => simpa [book2] using agree_replace h hfind hid hside hpx
  · cases side with
    | buy =>
      have hsub := (flat_removeAt_sublist book.bids px id).append_right
        (book.asks.flatMap (·.orders))
      simpa [book2, queueIds, dequeue, writeOrder, setLevels, levelsOf] using
        Sublist.nodup hsub h.nodup
    | sell =>
      have hsub := (flat_removeAt_sublist book.asks px id).append_left
        (book.bids.flatMap (·.orders))
      simpa [book2, queueIds, dequeue, writeOrder, setLevels, levelsOf] using
        Sublist.nodup hsub h.nodup


theorem front_of_best {levels : List Level} (hfull : noEmptyLevel levels) {px : Int}
    (hbest : bestPrice levels = some px) :
    ∃ id rest tail, levels = { price := px, orders := id :: rest } :: tail ∧
      frontAt levels px = some id := by
  cases levels with
  | nil => simp [bestPrice] at hbest
  | cons lvl tail =>
    simp [bestPrice] at hbest
    cases hbest
    have hne : lvl.orders ≠ [] := hfull lvl mem_cons_self
    cases hord : lvl.orders with
    | nil => exact absurd hord hne
    | cons id rest =>
      refine ⟨id, rest, tail, ?_, ?_⟩
      · rw [← hord]
      · simp [frontAt, hord]

theorem front_order (h : BookInv book) (side : Side) {px : Int}
    (hbest : bestPrice (levelsOf book side) = some px) :
    ∃ rid resting rest tail,
      levelsOf book side = { price := px, orders := rid :: rest } :: tail ∧
      frontAt (levelsOf book side) px = some rid ∧
      findOrder book.orders rid = some resting ∧
      resting.side = side ∧ resting.price = px ∧
      resting.status.isActive = true ∧ 0 < resting.remaining := by
  rcases front_of_best (side_full h side) hbest with ⟨rid, rest, tail, hshape, hfront⟩
  have hlev : ({ price := px, orders := rid :: rest } : Level) ∈ levelsOf book side := by
    simp [hshape]
  have hi : rid ∈ ({ price := px, orders := rid :: rest } : Level).orders := mem_cons_self
  rcases side_agree h side { price := px, orders := rid :: rest } hlev rid hi with
    ⟨resting, hf, hs, hp, ha, hr⟩
  refine ⟨rid, resting, rest, tail, hshape, hfront, hf, hs, ?_, ha, hr⟩
  · simpa using hp

theorem queueIds_dequeue_sub (book : Book) (side : Side) (px : Int) (id : Nat) :
    Sublist (queueIds (dequeue book side px id)) (queueIds book) := by
  cases side with
  | buy =>
    simpa [queueIds, dequeue, setLevels, levelsOf] using
      (flat_removeAt_sublist book.bids px id).append_right (book.asks.flatMap (·.orders))
  | sell =>
    simpa [queueIds, dequeue, setLevels, levelsOf] using
      (flat_removeAt_sublist book.asks px id).append_left (book.bids.flatMap (·.orders))

theorem stpConflict_off (a b : Order) : stpConflict a b .off = false := by
  cases ha : a.owner <;> cases hb : b.owner <;> simp [stpConflict, ha, hb]

theorem findOrder_upsert_self (orders : List (Nat × Order)) (o : Order) :
    findOrder (upsert orders o) o.id = some o := by
  simpa using findOrder_upsert orders o o.id

theorem cancelOrder_active {book : Book} {id : Nat} {o : Order}
    (hfind : findOrder book.orders id = some o) (hact : o.status.isActive = true) :
    cancelOrder book id =
      dequeue (writeOrder book { o with remaining := 0, status := .cancelled }) o.side o.price id := by
  simp [cancelOrder, hfind, hact]

theorem fuel_remaining_drop {book book' : Book} {incoming incoming' : Order}
    (hless : incoming'.remaining < incoming.remaining)
    (hq : queueLoad book' ≤ queueLoad book) :
    matchFuel book' incoming' < matchFuel book incoming := by
  simp only [matchFuel, queueLoad] at hq ⊢
  omega

theorem fuel_after_cancel {book : Book} {incoming o' : Order} {side : Side} {px : Int} {id : Nat}
    {rest : List Nat} {tail : List Level}
    (hshape : levelsOf book side = { price := px, orders := id :: rest } :: tail) :
    matchFuel (dequeue (writeOrder book o') side px id) incoming < matchFuel book incoming := by
  have hshape' : levelsOf (writeOrder book o') side =
      { price := px, orders := id :: rest } :: tail := by
    cases side <;> simpa [writeOrder, levelsOf] using hshape
  have hdrop := queueLoad_dequeue_front hshape'
  have hw : queueLoad (writeOrder book o') = queueLoad book := by simp [queueLoad, writeOrder]
  rw [hw] at hdrop
  simp only [matchFuel, queueLoad] at hdrop ⊢
  omega

theorem nextId_setLevels (book : Book) (side : Side) (levels : List Level) :
    (setLevels book side levels).nextOrderId = book.nextOrderId := by
  cases side <;> rfl

theorem orders_setLevels (book : Book) (side : Side) (levels : List Level) :
    (setLevels book side levels).orders = book.orders := by
  cases side <;> rfl

theorem nextId_dequeue (book : Book) (side : Side) (px : Int) (id : Nat) :
    (dequeue book side px id).nextOrderId = book.nextOrderId := by
  simp [dequeue, nextId_setLevels]

theorem orders_dequeue (book : Book) (side : Side) (px : Int) (id : Nat) :
    (dequeue book side px id).orders = book.orders := by
  simp [dequeue, orders_setLevels]

theorem nextId_emit (book : Book) (price : Int) (qty aggressor passive : Nat) (side : Side) :
    (emitTrade book price qty aggressor passive side).1.nextOrderId = book.nextOrderId := by
  simp [emitTrade]

theorem orders_emit (book : Book) (price : Int) (qty aggressor passive : Nat) (side : Side) :
    (emitTrade book price qty aggressor passive side).1.orders = book.orders := by
  simp [emitTrade]

theorem bookInv_emit (h : BookInv book) (price : Int) (qty aggressor passive : Nat) (side : Side) :
    BookInv (emitTrade book price qty aggressor passive side).1 := by
  simp only [emitTrade]
  exact ⟨h.bidOrder, h.askOrder, h.bidFull, h.askFull, h.agreeBid, h.agreeAsk, h.keys, h.nodup, h.below⟩

theorem agree_write (h : BookInv book) {o o' : Order} (side : Side)
    (hfind : findOrder book.orders o.id = some o) (hid : o'.id = o.id)
    (hside : o'.side = o.side) (hpx : o'.price = o.price)
    (hact : o'.status.isActive = true) (hrem : 0 < o'.remaining) :
    queuedActive (writeOrder book o') side := by
  intro level hl i hi
  have hl0 : level ∈ levelsOf book side := by
    cases side <;> simpa [writeOrder, levelsOf] using hl
  rcases side_agree h side level hl0 i hi with ⟨oq, hq, hsq, hpq, haq, hrq⟩
  have horders : (writeOrder book o').orders = upsert book.orders o' := by simp [writeOrder]
  by_cases hie : i = o'.id
  · have hoq : oq = o := by
      have hq' : findOrder book.orders o.id = some oq := by simpa [hie, hid] using hq
      rw [hfind] at hq'
      cases hq'
      rfl
    refine ⟨o', ?_, ?_, ?_, hact, hrem⟩
    · rw [horders, findOrder_upsert]
      simp [hie]
    · simpa [hside, hoq] using hsq
    · simpa [hpx, hoq] using hpq
  · refine ⟨oq, ?_, hsq, hpq, haq, hrq⟩
    · rw [horders, findOrder_upsert, ite_eq_right hie]
      exact hq

theorem bookInv_write_active (h : BookInv book) {o o' : Order}
    (hfind : findOrder book.orders o.id = some o) (hid : o'.id = o.id)
    (hside : o'.side = o.side) (hpx : o'.price = o.price)
    (hact : o'.status.isActive = true) (hrem : 0 < o'.remaining) :
    BookInv (writeOrder book o') := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · simpa [writeOrder, bestFirst] using h.bidOrder
  · simpa [writeOrder, bestFirst] using h.askOrder
  · simpa [writeOrder] using h.bidFull
  · simpa [writeOrder] using h.askFull
  · exact agree_write h .buy hfind hid hside hpx hact hrem
  · exact agree_write h .sell hfind hid hside hpx hact hrem
  · simpa [writeOrder] using keysMatch_upsert book.orders o' h.keys
  · simpa [writeOrder, queueIds] using h.nodup
  · intro p hp
    have hlt : o.id < book.nextOrderId := h.below (o.id, o) (findOrder_mem h.keys hfind)
    have ho' : o'.id < book.nextOrderId := by simpa [hid] using hlt
    exact below_upsert h.below ho' p (by simpa [writeOrder] using hp)

/-- One match step keeps the book well formed and spends fuel when it continues. -/
structure StepKeep (book : Book) (incoming : Order) (s : Step) : Prop where
  inv : BookInv s.book
  nextId : s.book.nextOrderId = book.nextOrderId
  queues : Sublist (queueIds s.book) (queueIds book)
  sameId : s.incoming.id = incoming.id
  kept : ∀ bound : Nat, (∀ p ∈ book.orders, p.1 < bound) → ∀ p ∈ s.book.orders, p.1 < bound
  fuel : s.go = true → matchFuel s.book s.incoming < matchFuel book incoming

theorem stepKeep_of {book book' : Book} {incoming incoming' : Order} {policy : StpPolicy}
    {stpFlag stp : Bool} {trade : Option Trade} {go : Bool}
    (hs : step book incoming policy stpFlag =
      { book := book', incoming := incoming', stp, trade, go })
    (hinv : BookInv book') (hnext : book'.nextOrderId = book.nextOrderId)
    (hqueues : Sublist (queueIds book') (queueIds book)) (hid : incoming'.id = incoming.id)
    (hkept : ∀ bound, (∀ p ∈ book.orders, p.1 < bound) → ∀ p ∈ book'.orders, p.1 < bound)
    (hfuel : go = true → matchFuel book' incoming' < matchFuel book incoming) :
    StepKeep book incoming (step book incoming policy stpFlag) := by
  rw [hs]
  exact ⟨hinv, hnext, hqueues, hid, hkept, hfuel⟩

theorem stop_step_keep {book : Book} {incoming incoming' : Order} {policy : StpPolicy}
    {stpFlag stp : Bool}
    (h : BookInv book)
    (hs : step book incoming policy stpFlag =
      { book, incoming := incoming', stp, trade := none, go := false })
    (hid : incoming'.id = incoming.id) :
    StepKeep book incoming (step book incoming policy stpFlag) :=
  stepKeep_of hs h rfl (Sublist.refl _) hid (fun _ hb => hb) (fun hgo => by cases hgo)

theorem cancel_step_keep (h : BookInv book) (incoming resting : Order) {policy : StpPolicy}
    {px : Int} {rid : Nat} {rest : List Nat} {tail : List Level}
    (hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hfind : findOrder book.orders rid = some resting)
    (hside : resting.side = incoming.side.opposite) (hpx : resting.price = px)
    (hact : resting.status.isActive = true)
    (hs : step book incoming policy false =
      { book := cancelOrder book rid, incoming, stp := false, trade := none, go := true }) :
    StepKeep book incoming (step book incoming policy false) := by
  let o' : Order := { resting with remaining := 0, status := .cancelled }
  have hidr : resting.id = rid := findOrder_id h.keys hfind
  have hid' : o'.id = rid := by
    unfold o'
    exact hidr
  have hc : cancelOrder book rid =
      dequeue (writeOrder book o') incoming.side.opposite px rid := by
    unfold o'
    simp [cancelOrder_active hfind hact, hside, hpx]
  have hinv := bookInv_replace_out h hfind hid' hside hpx
  have hnext : (cancelOrder book rid).nextOrderId = book.nextOrderId := by
    rw [hc]
    simp [nextId_dequeue, writeOrder]
  have hqueues : Sublist (queueIds (cancelOrder book rid)) (queueIds book) := by
    simpa [hc, queueIds, writeOrder] using
      queueIds_dequeue_sub (writeOrder book o') incoming.side.opposite px rid
  have hkept : ∀ bound, (∀ p ∈ book.orders, p.1 < bound) →
      ∀ p ∈ (cancelOrder book rid).orders, p.1 < bound := by
    intro bound hb p hp
    have hp' : p ∈ upsert book.orders o' := by
      rw [hc] at hp
      simpa [orders_dequeue, writeOrder] using hp
    have hlt : rid < bound := hb (rid, resting) (findOrder_mem h.keys hfind)
    have ho' : o'.id < bound := by
      unfold o'
      rw [hidr]
      exact hlt
    exact below_upsert hb ho' p hp'
  have hfuel : matchFuel (cancelOrder book rid) incoming < matchFuel book incoming := by
    simpa [hc] using fuel_after_cancel (incoming := incoming) (o' := o') hshape
  exact stepKeep_of hs (by simpa [hc] using hinv) hnext hqueues rfl hkept (fun _ => hfuel)

theorem fill_step_keep (h : BookInv book) (incoming resting : Order) (policy : StpPolicy)
    {px : Int} {rid n : Nat}
    (hsucc : incoming.remaining = Nat.succ n)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (hside : resting.side = incoming.side.opposite) (hpx : resting.price = px)
    (hact : resting.status.isActive = true) (hrest : 0 < resting.remaining)
    (hconf : stpConflict incoming resting policy = false) :
    StepKeep book incoming (step book incoming policy false) := by
  let opp := incoming.side.opposite
  let qty := min incoming.remaining resting.remaining
  have hqty : 0 < qty := Nat.lt_min.mpr ⟨by simp [hsucc], hrest⟩
  have hle : qty ≤ incoming.remaining := Nat.min_le_left _ _
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
  have hidr : resting.id = rid := findOrder_id h.keys hfind
  have hless : incoming'.remaining < incoming.remaining := by
    have hsub : incoming'.remaining = incoming.remaining - qty := by simp [incoming', fill]
    simpa [hsub] using Nat.sub_lt_of_pos_le hqty hle
  have hE : BookInv emitted.1 := bookInv_emit h px qty incoming.id rid incoming.side
  have hfindE : findOrder emitted.1.orders rid = some resting := by
    simpa [emitted, emitTrade] using hfind
  have hid' : resting'.id = rid := by simp [resting', fill, hidr]
  have hside' : resting'.side = resting.side := by simp [resting', fill]
  have hpx' : resting'.price = resting.price := by simp [resting', fill]
  have hpack : BookInv book2 ∧ book2.nextOrderId = book.nextOrderId ∧
      Sublist (queueIds book2) (queueIds book) ∧
      (∀ bound, (∀ p ∈ book.orders, p.1 < bound) → ∀ p ∈ book2.orders, p.1 < bound) ∧
      queueLoad book2 ≤ queueLoad book := by
    cases hdead : (resting'.remaining == 0)
    · have hneR : resting'.remaining ≠ 0 := by simpa using hdead
      have hposR : 0 < resting'.remaining := Nat.pos_of_ne_zero hneR
      have hneSub : (resting.remaining - qty == 0) = false := by
        simpa [resting', fill] using hdead
      have hact' : resting'.status.isActive = true := by
        simp only [resting', fill]
        rw [hneSub]
        simp [OrderStatus.isActive]
      have hfindId : findOrder emitted.1.orders resting.id = some resting := by
        simpa [hidr] using hfindE
      have hinv := bookInv_write_active hE hfindId (by simp [resting', fill]) hside' hpx' hact' hposR
      have hkept : ∀ bound, (∀ p ∈ book.orders, p.1 < bound) → ∀ p ∈ book2.orders, p.1 < bound := by
        intro bound hb p hp
        have hp' : p ∈ upsert book.orders resting' := by
          simpa [book2, hdead, book1, emitted, emitTrade, writeOrder] using hp
        have hlt : rid < bound := hb (rid, resting) (findOrder_mem h.keys hfind)
        have ho' : resting'.id < bound := by simpa [resting', fill, hidr] using hlt
        exact below_upsert hb ho' p hp'
      refine ⟨?_, ?_, ?_, hkept, ?_⟩
      · simpa [book2, hdead] using hinv
      · simp [book2, hdead, book1, emitted, emitTrade, writeOrder]
      · simp [book2, hdead, book1, queueIds, writeOrder, emitted, emitTrade]
      · have hqE := queueLoad_emit book px qty incoming.id rid incoming.side
        have hqW := queueLoad_write emitted.1 resting'
        simp [book2, hdead, book1, emitted, hqE, hqW]
    · have hinv := bookInv_replace_out hE hfindE hid' hside hpx
      have hkept : ∀ bound, (∀ p ∈ book.orders, p.1 < bound) → ∀ p ∈ book2.orders, p.1 < bound := by
        intro bound hb p hp
        have hp' : p ∈ upsert book.orders resting' := by
          unfold book2 book1 at hp
          rw [hdead] at hp
          simpa [orders_dequeue, writeOrder, emitted, orders_emit] using hp
        have hlt : rid < bound := hb (rid, resting) (findOrder_mem h.keys hfind)
        have ho' : resting'.id < bound := by simpa [resting', fill, hidr] using hlt
        exact below_upsert hb ho' p hp'
      refine ⟨?_, ?_, ?_, hkept, ?_⟩
      · simpa [book2, book1, hdead] using hinv
      · unfold book2 book1
        rw [hdead]
        simp [nextId_dequeue, writeOrder, emitted, nextId_emit]
      · simpa [book2, hdead, book1, queueIds, writeOrder, emitted, emitTrade] using
          queueIds_dequeue_sub book1 opp px rid
      · have hqE := queueLoad_emit book px qty incoming.id rid incoming.side
        have hqW := queueLoad_write emitted.1 resting'
        have hqD := queueLoad_dequeue_le book1 opp px rid
        simp [book1, emitted, hqE, hqW] at hqD
        simpa [book2, hdead] using hqD
  have hfuel := fuel_remaining_drop hless hpack.2.2.2.2
  exact stepKeep_of hs hpack.1 hpack.2.1 hpack.2.2.1 (by simp [incoming', fill]) hpack.2.2.2.1
    (fun _ => hfuel)

theorem step_progress (h : BookInv book) (incoming : Order) (policy : StpPolicy) (stp : Bool) :
    StepKeep book incoming (step book incoming policy stp) := by
  cases stp
  case true =>
    have hs : step book incoming policy true =
        { book, incoming, stp := true, trade := none, go := false } := by
      simp [step]
    exact stop_step_keep h hs rfl
  case false =>
    cases hsucc : incoming.remaining
    case zero =>
      have hs : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc]
      exact stop_step_keep h hs rfl
    case succ n =>
      let opp := incoming.side.opposite
      cases hbest : bestPrice (levelsOf book opp)
      case none =>
        have hs : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hsucc, hbest, opp]
        exact stop_step_keep h hs rfl
      case some px =>
        cases hcross : pricesCross incoming.side incoming.price px
        case false =>
          have hs : step book incoming policy false =
              { book, incoming, stp := false, trade := none, go := false } := by
            simp [step, hsucc, hbest, hcross, opp]
          exact stop_step_keep h hs rfl
        case true =>
          rcases front_order h opp hbest with
            ⟨rid, resting, _rest, _tail, hshape, hfront, hfind, hside, hpx, hact, hrest⟩
          have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
          cases policy
          case off =>
            exact fill_step_keep h incoming resting .off hsucc hbest hcross hfront hfind hside hpx
              hact hrest (stpConflict_off incoming resting)
          case cancelNewest =>
            cases hconf : stpConflict incoming resting .cancelNewest
            case false =>
              exact fill_step_keep h incoming resting .cancelNewest hsucc hbest hcross hfront hfind
                hside hpx hact hrest hconf
            case true =>
              have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
              have hs : step book incoming .cancelNewest false =
                  { book, incoming := stopIncoming incoming, stp := true, trade := none, go := false } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq, opp]
              exact stop_step_keep h hs (by simp [stopIncoming])
          case cancelOldest =>
            cases hconf : stpConflict incoming resting .cancelOldest
            case false =>
              exact fill_step_keep h incoming resting .cancelOldest hsucc hbest hcross hfront hfind
                hside hpx hact hrest hconf
            case true =>
              have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
              have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
              have hs : step book incoming .cancelOldest false =
                  { book := cancelOrder book rid, incoming, stp := false, trade := none, go := true } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hsame, opp]
              exact cancel_step_keep h incoming resting hshape hfind hside hpx hact hs
          case decrementAndCancel =>
            cases hconf : stpConflict incoming resting .decrementAndCancel
            case false =>
              exact fill_step_keep h incoming resting .decrementAndCancel hsucc hbest hcross hfront
                hfind hside hpx hact hrest hconf
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
                exact stop_step_keep h hs (by simp [stopIncoming])
              · have hnotN : ¬ n + 1 < resting.remaining := by simpa [hsucc] using hltR
                have hs : step book incoming .decrementAndCancel false =
                    { book := cancelOrder book rid, incoming, stp := false, trade := none,
                      go := true } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hnotOld, hdec, hnotN, opp]
                exact cancel_step_keep h incoming resting hshape hfind hside hpx hact hs

theorem agree_insertBid (orders : List (Nat × Order)) (o : Order)
    (hfind : findOrder orders o.id = some o) (hside : o.side = .buy)
    (hact : o.status.isActive = true) (hrem : 0 < o.remaining)
    (levels : List Level)
    (hagree : ∀ level ∈ levels, ∀ i ∈ level.orders,
      ∃ ord, findOrder orders i = some ord ∧ ord.side = .buy ∧ ord.price = level.price ∧
        ord.status.isActive = true ∧ 0 < ord.remaining) :
    ∀ level ∈ insertBid levels o.price o.id, ∀ i ∈ level.orders,
      ∃ ord, findOrder orders i = some ord ∧ ord.side = .buy ∧ ord.price = level.price ∧
        ord.status.isActive = true ∧ 0 < ord.remaining := by
  induction levels with
  | nil =>
    intro level hl i hi
    simp [insertBid] at hl
    subst hl
    simp at hi
    subst hi
    exact ⟨o, hfind, hside, rfl, hact, hrem⟩
  | cons lvl tail ih =>
    intro level hl i hi
    simp only [insertBid] at hl
    split at hl
    · rcases mem_cons.mp hl with rfl | hl
      · simp at hi
        subst hi
        exact ⟨o, hfind, hside, rfl, hact, hrem⟩
      · exact hagree level hl i hi
    · split at hl
      · rename_i heq
        have hpx : o.price = lvl.price := by simpa using heq
        rcases mem_cons.mp hl with rfl | hl
        · rcases mem_append.mp hi with hi | hi
          · exact hagree lvl mem_cons_self i hi
          · simp at hi
            subst hi
            exact ⟨o, hfind, hside, hpx, hact, hrem⟩
        · exact hagree level (mem_cons_of_mem lvl hl) i hi
      · rcases mem_cons.mp hl with rfl | hl
        · exact hagree level mem_cons_self i hi
        · exact ih (fun level hm i hi => hagree level (mem_cons_of_mem lvl hm) i hi) level hl i hi

theorem agree_insertAsk (orders : List (Nat × Order)) (o : Order)
    (hfind : findOrder orders o.id = some o) (hside : o.side = .sell)
    (hact : o.status.isActive = true) (hrem : 0 < o.remaining)
    (levels : List Level)
    (hagree : ∀ level ∈ levels, ∀ i ∈ level.orders,
      ∃ ord, findOrder orders i = some ord ∧ ord.side = .sell ∧ ord.price = level.price ∧
        ord.status.isActive = true ∧ 0 < ord.remaining) :
    ∀ level ∈ insertAsk levels o.price o.id, ∀ i ∈ level.orders,
      ∃ ord, findOrder orders i = some ord ∧ ord.side = .sell ∧ ord.price = level.price ∧
        ord.status.isActive = true ∧ 0 < ord.remaining := by
  induction levels with
  | nil =>
    intro level hl i hi
    simp [insertAsk] at hl
    subst hl
    simp at hi
    subst hi
    exact ⟨o, hfind, hside, rfl, hact, hrem⟩
  | cons lvl tail ih =>
    intro level hl i hi
    simp only [insertAsk] at hl
    split at hl
    · rcases mem_cons.mp hl with rfl | hl
      · simp at hi
        subst hi
        exact ⟨o, hfind, hside, rfl, hact, hrem⟩
      · exact hagree level hl i hi
    · split at hl
      · rename_i heq
        have hpx : o.price = lvl.price := by simpa using heq
        rcases mem_cons.mp hl with rfl | hl
        · rcases mem_append.mp hi with hi | hi
          · exact hagree lvl mem_cons_self i hi
          · simp at hi
            subst hi
            exact ⟨o, hfind, hside, hpx, hact, hrem⟩
        · exact hagree level (mem_cons_of_mem lvl hl) i hi
      · rcases mem_cons.mp hl with rfl | hl
        · exact hagree level mem_cons_self i hi
        · exact ih (fun level hm i hi => hagree level (mem_cons_of_mem lvl hm) i hi) level hl i hi

theorem agree_write_fresh (h : BookInv book) (side : Side) {o : Order}
    (hfresh : o.id ∉ (levelsOf book side).flatMap (·.orders)) :
    queuedActive (writeOrder book o) side := by
  intro level hl i hi
  have hl0 : level ∈ levelsOf book side := by
    cases side <;> simpa [writeOrder, levelsOf] using hl
  have hne : i ≠ o.id := by
    intro heq
    exact hfresh (mem_flatMap.mpr ⟨level, hl0, heq ▸ hi⟩)
  rcases side_agree h side level hl0 i hi with ⟨oq, hq, hsq, hpq, haq, hrq⟩
  refine ⟨oq, ?_, hsq, hpq, haq, hrq⟩
  · have horders : (writeOrder book o).orders = upsert book.orders o := by simp [writeOrder]
    rw [horders, findOrder_upsert, ite_eq_right hne]
    exact hq

theorem bookInv_write_fresh (h : BookInv book) {o : Order}
    (hid : o.id < book.nextOrderId) (hfresh : o.id ∉ queueIds book) :
    BookInv (writeOrder book o) := by
  have hnotBid : o.id ∉ book.bids.flatMap (·.orders) := by
    intro hm
    exact hfresh (mem_append_left _ hm)
  have hnotAsk : o.id ∉ book.asks.flatMap (·.orders) := by
    intro hm
    exact hfresh (mem_append_right _ hm)
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · simpa [writeOrder, bestFirst] using h.bidOrder
  · simpa [writeOrder, bestFirst] using h.askOrder
  · simpa [writeOrder] using h.bidFull
  · simpa [writeOrder] using h.askFull
  · exact agree_write_fresh h .buy (by simpa [levelsOf] using hnotBid)
  · exact agree_write_fresh h .sell (by simpa [levelsOf] using hnotAsk)
  · simpa [writeOrder] using keysMatch_upsert book.orders o h.keys
  · simpa [writeOrder, queueIds] using h.nodup
  · intro p hp
    have hlt := below_upsert h.below hid p (by simpa [writeOrder] using hp)
    simpa [writeOrder] using hlt

theorem bookInv_enqueue (h : BookInv book) {o stored : Order}
    (hfind : findOrder book.orders o.id = some stored) (hid : stored.id = o.id)
    (hside : stored.side = o.side) (hpx : stored.price = o.price)
    (hact : stored.status.isActive = true) (hrem : 0 < stored.remaining)
    (hfresh : o.id ∉ queueIds book) :
    BookInv (enqueue book o) := by
  have hpair := nodup_append.mp (by simpa [queueIds] using h.nodup)
  have hnotBid : o.id ∉ book.bids.flatMap (·.orders) := by
    intro hm
    exact hfresh (mem_append_left _ hm)
  have hnotAsk : o.id ∉ book.asks.flatMap (·.orders) := by
    intro hm
    exact hfresh (mem_append_right _ hm)
  have hfindS : findOrder book.orders stored.id = some stored := by simpa [hid] using hfind
  cases hsideO : o.side with
  | buy =>
    have hsideB : stored.side = .buy := by simpa [hsideO] using hside
    have hagree := agree_insertBid book.orders stored hfindS hsideB hact hrem book.bids
      (by simpa [queuedActive, levelsOf] using h.agreeBid)
    have hnew := nodup_insertBid book.bids o.price o.id hpair.1 hnotBid
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
    · have hbest := insertBid_best book.bids o.price o.id (by simpa [bestFirst] using h.bidOrder)
      simpa [enqueue, hsideO, bestFirst] using hbest
    · simpa [enqueue, hsideO, bestFirst] using h.askOrder
    · simpa [enqueue, hsideO] using noEmpty_insertBid h.bidFull o.price o.id
    · simpa [enqueue, hsideO] using h.askFull
    · simpa [enqueue, hsideO, queuedActive, levelsOf, hid, hpx] using hagree
    · simpa [enqueue, hsideO, queuedActive, levelsOf] using h.agreeAsk
    · simpa [enqueue, hsideO, keysMatch] using h.keys
    · have hdis : ∀ a ∈ (insertBid book.bids o.price o.id).flatMap (·.orders),
          ∀ b ∈ book.asks.flatMap (·.orders), a ≠ b := by
        intro a ha b hb
        rcases mem_insertBid_flat ha with rfl | ha
        · intro heq
          exact hnotAsk (heq ▸ hb)
        · exact hpair.2.2 a ha b hb
      simpa [enqueue, hsideO, queueIds] using nodup_append.mpr ⟨hnew, hpair.2.1, hdis⟩
    · simpa [enqueue, hsideO] using h.below
  | sell =>
    have hsideS : stored.side = .sell := by simpa [hsideO] using hside
    have hagree := agree_insertAsk book.orders stored hfindS hsideS hact hrem book.asks
      (by simpa [queuedActive, levelsOf] using h.agreeAsk)
    have hnew := nodup_insertAsk book.asks o.price o.id hpair.2.1 hnotAsk
    refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
    · simpa [enqueue, hsideO, bestFirst] using h.bidOrder
    · have hbest := insertAsk_best book.asks o.price o.id (by simpa [bestFirst] using h.askOrder)
      simpa [enqueue, hsideO, bestFirst] using hbest
    · simpa [enqueue, hsideO] using h.bidFull
    · simpa [enqueue, hsideO] using noEmpty_insertAsk h.askFull o.price o.id
    · simpa [enqueue, hsideO, queuedActive, levelsOf] using h.agreeBid
    · simpa [enqueue, hsideO, queuedActive, levelsOf, hid, hpx] using hagree
    · simpa [enqueue, hsideO, keysMatch] using h.keys
    · have hdis : ∀ a ∈ book.bids.flatMap (·.orders),
          ∀ b ∈ (insertAsk book.asks o.price o.id).flatMap (·.orders), a ≠ b := by
        intro a ha b hb
        rcases mem_insertAsk_flat hb with rfl | hb
        · intro heq
          exact hnotBid (heq.symm ▸ ha)
        · exact hpair.2.2 a ha b hb
      simpa [enqueue, hsideO, queueIds] using nodup_append.mpr ⟨hpair.1, hnew, hdis⟩
    · simpa [enqueue, hsideO] using h.below

theorem id_not_queued (h : BookInv book) {id : Nat}
    (hbelow : ∀ p ∈ book.orders, p.1 < id) : id ∉ queueIds book := by
  intro hq
  rcases mem_append.mp hq with hq | hq
  · rcases mem_flatMap.mp hq with ⟨level, hl, hi⟩
    rcases h.agreeBid level (by simpa [levelsOf] using hl) id hi with ⟨o, hf, _, _, _, _⟩
    have hmem := findOrder_mem h.keys hf
    exact absurd (hbelow (id, o) hmem) (Nat.lt_irrefl id)
  · rcases mem_flatMap.mp hq with ⟨level, hl, hi⟩
    rcases h.agreeAsk level (by simpa [levelsOf] using hl) id hi with ⟨o, hf, _, _, _, _⟩
    have hmem := findOrder_mem h.keys hf
    exact absurd (hbelow (id, o) hmem) (Nat.lt_irrefl id)

theorem bookInv_settle (h : BookInv book) (o : Order) (stp : Bool)
    (hbelow : ∀ p ∈ book.orders, p.1 < o.id) (hnext : o.id < book.nextOrderId) :
    BookInv (settle book o stp) := by
  have hfresh : o.id ∉ queueIds book := id_not_queued h hbelow
  cases hstp : stp
  · cases hzero : (o.remaining == 0)
    · cases htif : (o.timeInForce == .gtc)
      · simpa [settle, hstp, hzero, htif] using
          bookInv_write_fresh (o := { o with remaining := 0, status := .cancelled }) h hnext hfresh
      · let status := if o.filled > 0 then OrderStatus.partiallyFilled else OrderStatus.new
        let o' : Order := { o with status := status }
        have hact : o'.status.isActive = true := by
          simp only [o']
          unfold status
          split <;> simp [OrderStatus.isActive]
        have hrem0 : o.remaining ≠ 0 := by simpa using hzero
        have hrem : 0 < o'.remaining := by simpa [o'] using Nat.pos_of_ne_zero hrem0
        have hW := bookInv_write_fresh h (o := o') hnext (by simpa [queueIds, writeOrder] using hfresh)
        have hfind : findOrder (writeOrder book o').orders o.id = some o' := by
          simpa [writeOrder] using findOrder_upsert_self book.orders o'
        have hfreshW : o.id ∉ queueIds (writeOrder book o') := by
          simpa [queueIds, writeOrder] using hfresh
        have hinv := bookInv_enqueue hW hfind rfl (by simp [o']) (by simp [o']) hact hrem hfreshW
        simpa [settle, hstp, hzero, htif, status, o'] using hinv
    · simpa [settle, hstp, hzero] using
        bookInv_write_fresh (o := { o with status := .filled }) h hnext hfresh
  · simpa [settle, hstp] using
      bookInv_write_fresh (o := { o with status := .cancelled }) h hnext hfresh

theorem bookInv_alloc (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) :
    BookInv (alloc book side price quantity tif owner).1 ∧
      (alloc book side price quantity tif owner).2.id = book.nextOrderId ∧
      (∀ p ∈ (alloc book side price quantity tif owner).1.orders,
        p.1 < (alloc book side price quantity tif owner).2.id) ∧
      (alloc book side price quantity tif owner).2.id <
        (alloc book side price quantity tif owner).1.nextOrderId := by
  have horders : (alloc book side price quantity tif owner).1.orders = book.orders := by
    simp [alloc]
  have hnext : (alloc book side price quantity tif owner).1.nextOrderId = book.nextOrderId + 1 := by
    simp [alloc]
  have hid : (alloc book side price quantity tif owner).2.id = book.nextOrderId := by
    simp [alloc]
  have hbids : (alloc book side price quantity tif owner).1.bids = book.bids := by simp [alloc]
  have hasks : (alloc book side price quantity tif owner).1.asks = book.asks := by simp [alloc]
  refine ⟨?_, hid, ?_, ?_⟩
  · refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
    · simpa [hbids, bestFirst] using h.bidOrder
    · simpa [hasks, bestFirst] using h.askOrder
    · simpa [hbids] using h.bidFull
    · simpa [hasks] using h.askFull
    · simpa [hbids, horders, queuedActive, levelsOf] using h.agreeBid
    · simpa [hasks, horders, queuedActive, levelsOf] using h.agreeAsk
    · simpa [horders] using h.keys
    · simpa [hbids, hasks, queueIds] using h.nodup
    · intro p hp
      have hlt := h.below p (by simpa [horders] using hp)
      have hlt' : p.1 < book.nextOrderId + 1 := by omega
      simpa [hnext] using hlt'
  · intro p hp
    simpa [hid] using h.below p (by simpa [horders] using hp)
  · simp [hid, hnext]

theorem matchLoop_keep (fuel : Nat) (h : BookInv book) (incoming : Order) (policy : StpPolicy)
    (rev : List Trade) (stp : Bool)
    (hbelow : ∀ p ∈ book.orders, p.1 < incoming.id)
    (hnext : incoming.id < book.nextOrderId) :
    BookInv (matchLoop fuel book incoming policy rev stp).1 ∧
      (matchLoop fuel book incoming policy rev stp).2.1.id = incoming.id ∧
      (∀ p ∈ (matchLoop fuel book incoming policy rev stp).1.orders, p.1 < incoming.id) ∧
      (matchLoop fuel book incoming policy rev stp).1.nextOrderId = book.nextOrderId := by
  induction fuel generalizing book incoming rev stp with
  | zero =>
    refine ⟨?_, ?_, ?_, ?_⟩
    · simpa [matchLoop] using h
    · simp [matchLoop]
    · simpa [matchLoop] using hbelow
    · simp [matchLoop]
  | succ n ih =>
    have hs := step_progress h incoming policy stp
    cases hgo : (step book incoming policy stp).go
    · simp [matchLoop, hgo]
      refine ⟨?_, ?_, ?_, ?_⟩
      · simpa using hs.inv
      · simpa using hs.sameId
      · simpa using hs.kept incoming.id hbelow
      · simpa using hs.nextId
    · have hkept0 := hs.kept incoming.id hbelow
      have hkept : ∀ p ∈ (step book incoming policy stp).book.orders,
          p.1 < (step book incoming policy stp).incoming.id := by
        intro p hp
        simpa [hs.sameId] using hkept0 p hp
      have hnext' : (step book incoming policy stp).incoming.id <
          (step book incoming policy stp).book.nextOrderId := by
        simpa [hs.nextId, hs.sameId] using hnext
      have hrec := ih hs.inv (step book incoming policy stp).incoming
        (match (step book incoming policy stp).trade with
          | none => rev
          | some t => t :: rev)
        (step book incoming policy stp).stp hkept hnext'
      simp only [matchLoop, hgo]
      refine ⟨hrec.1, hrec.2.1.trans hs.sameId, ?_, hrec.2.2.2.trans hs.nextId⟩
      · intro p hp
        exact hs.sameId ▸ hrec.2.2.1 p hp

theorem bookInv_submit (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) (policy : StpPolicy) :
    BookInv (submit book side price quantity tif owner policy).1 := by
  unfold submit
  split
  · simpa using (bookInv_alloc h side price quantity tif owner).1
  · let out := alloc book side price quantity tif owner
    have halloc := bookInv_alloc h side price quantity tif owner
    let looped := matchLoop (matchFuel out.1 out.2) out.1 out.2 policy [] false
    have hloop := matchLoop_keep (book := out.1) (matchFuel out.1 out.2) halloc.1 out.2 policy [] false
      halloc.2.2.1 halloc.2.2.2
    have hbelow : ∀ p ∈ looped.1.orders, p.1 < looped.2.1.id := by
      intro p hp
      have hlt := hloop.2.2.1 p hp
      rw [hloop.2.1]
      exact hlt
    have hnext : looped.2.1.id < looped.1.nextOrderId := by
      rw [hloop.2.1, hloop.2.2.2]
      exact halloc.2.2.2
    have hsettle := bookInv_settle hloop.1 looped.2.1 looped.2.2.2 hbelow hnext
    simpa [out, looped] using hsettle

theorem bookInv_cancel (h : BookInv book) (id : Nat) : BookInv (cancelOrder book id) := by
  cases hfind : findOrder book.orders id with
  | none =>
    simp [cancelOrder, hfind]
    exact h
  | some o =>
    cases hact : o.status.isActive
    · simp [cancelOrder, hfind, hact]
      exact h
    · let o' : Order := { o with remaining := 0, status := .cancelled }
      have hid : o'.id = id := by
        have hid0 := findOrder_id h.keys hfind
        simpa [o'] using hid0
      have hinv := bookInv_replace_out h hfind hid rfl rfl
      simpa [cancelOrder, hfind, hact, o'] using hinv

theorem bookInv_apply (h : BookInv book) (event : Event) : BookInv (apply book event).1 := by
  cases event with
  | submitLimit side price quantity tif owner policy =>
    simpa [apply] using bookInv_submit h side price quantity tif owner policy
  | submitMarket side quantity owner policy =>
    cases side with
    | buy => simpa [apply] using bookInv_submit h .buy i64Max quantity .ioc owner policy
    | sell => simpa [apply] using bookInv_submit h .sell i64Min quantity .ioc owner policy
  | cancel id =>
    simpa [apply] using bookInv_cancel h id

theorem foldl_apply_inv (h : BookInv book) (events : List Event) :
    BookInv (events.foldl (fun b ev => (apply b ev).1) book) := by
  induction events generalizing book with
  | nil => simpa using h
  | cons event rest ih => exact ih (bookInv_apply h event)

theorem reach_inv (events : List Event) : BookInv (reach events) := by
  simpa [reach] using foldl_apply_inv bookInv_empty events

theorem reachable_inv {book : Book} (h : Reachable book) : BookInv book := by
  rcases h with ⟨events, rfl⟩
  exact reach_inv events

/-- A queued id is an active order at that level's price. -/
theorem queued_id_active {book : Book} (h : Reachable book) {side : Side} {level : Level}
    {id : Nat} (hlevel : level ∈ levelsOf book side) (hid : id ∈ level.orders) :
    ∃ o, findOrder book.orders id = some o ∧
      o.side = side ∧ o.price = level.price ∧
      o.status.isActive = true ∧ 0 < o.remaining :=
  side_agree (reachable_inv h) side level hlevel id hid

end Nanobook
