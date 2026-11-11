import Nanobook.Proofs.WellFormed

namespace Nanobook

theorem matchLoop_fuel_idx (n : Nat) :
    ∀ book, BookInv book → ∀ incoming, matchFuel book incoming = n →
      ∀ (policy : StpPolicy) (rev : List Trade) (stp : Bool) (k : Nat),
        matchLoop (n + k) book incoming policy rev stp =
        matchLoop n book incoming policy rev stp := by
  induction n using Nat.strongRecOn with
  | ind n ih =>
    intro book hinv incoming hn policy rev stp k
    cases n with
    | zero =>
      have hrem : incoming.remaining = 0 := by
        simp only [matchFuel] at hn
        omega
      cases k with
      | zero => rfl
      | succ m =>
        cases stp <;> simp [matchLoop, step, hrem]
    | succ m =>
      rw [Nat.succ_add]
      cases hgo : (step book incoming policy stp).go
      · simp [matchLoop, hgo]
      · have hs := step_progress hinv incoming policy stp
        have hlt : matchFuel (step book incoming policy stp).book
            (step book incoming policy stp).incoming < Nat.succ m := by
          simpa [hn] using hs.fuel hgo
        have hle : matchFuel (step book incoming policy stp).book
            (step book incoming policy stp).incoming ≤ m := Nat.le_of_lt_succ hlt
        have hm : matchFuel (step book incoming policy stp).book
              (step book incoming policy stp).incoming +
            (m - matchFuel (step book incoming policy stp).book
              (step book incoming policy stp).incoming) = m :=
          Nat.add_sub_of_le hle
        have hmk : matchFuel (step book incoming policy stp).book
              (step book incoming policy stp).incoming +
            (m - matchFuel (step book incoming policy stp).book
              (step book incoming policy stp).incoming + k) = m + k := by
          omega
        have hih := ih (matchFuel (step book incoming policy stp).book
            (step book incoming policy stp).incoming) hlt
          (step book incoming policy stp).book hs.inv
          (step book incoming policy stp).incoming rfl policy
          (match (step book incoming policy stp).trade with
            | none => rev
            | some t => t :: rev)
          (step book incoming policy stp).stp
        simp [matchLoop, hgo]
        rw [← hmk]
        conv =>
          rhs
          rw [← hm]
        exact (hih (m - matchFuel (step book incoming policy stp).book
            (step book incoming policy stp).incoming + k)).trans
          (hih (m - matchFuel (step book incoming policy stp).book
            (step book incoming policy stp).incoming)).symm

theorem matchLoop_fuel (h : BookInv book) (incoming : Order) (policy : StpPolicy)
    (rev : List Trade) (stp : Bool) (k : Nat) :
    matchLoop (matchFuel book incoming + k) book incoming policy rev stp =
    matchLoop (matchFuel book incoming) book incoming policy rev stp :=
  matchLoop_fuel_idx (matchFuel book incoming) book h incoming rfl policy rev stp k

theorem law6_holds : law6 := by
  intro book hReach incoming policy k
  exact matchLoop_fuel (reachable_inv hReach) incoming policy [] false k

end Nanobook
