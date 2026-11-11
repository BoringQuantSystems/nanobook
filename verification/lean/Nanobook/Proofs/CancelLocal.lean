/-
Cancel of one id touches only that order.
-/

import Nanobook.Proofs.WellFormed

namespace Nanobook

/-- An active cancel keeps every other order and drops only that id. -/
theorem cancel_keeps_others (h : BookInv book) {x : Nat} {o : Order}
    (hfind : findOrder book.orders x = some o) (hact : o.status.isActive = true) :
    let book' := cancelOrder book x
    book'.nextOrderId = book.nextOrderId ∧
    book'.nextTradeId = book.nextTradeId ∧
    book'.nextTimestamp = book.nextTimestamp ∧
    findOrder book'.orders x = some { o with remaining := 0, status := .cancelled } ∧
    (∀ y, y ≠ x → findOrder book'.orders y = findOrder book.orders y) ∧
    levelsOf book' o.side = removeAt (levelsOf book o.side) o.price x ∧
    levelsOf book' o.side.opposite = levelsOf book o.side.opposite := by
  have hid : o.id = x := findOrder_id h.keys hfind
  let o' : Order := { o with remaining := 0, status := .cancelled }
  have hbook : cancelOrder book x = dequeue (writeOrder book o') o.side o.price x := by
    simpa [o'] using cancelOrder_active hfind hact
  have hid' : o'.id = x := by simp [o', hid]
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · rw [hbook]; cases o.side <;> rfl
  · rw [hbook]; cases o.side <;> rfl
  · rw [hbook]; cases o.side <;> rfl
  · rw [hbook, orders_dequeue, writeOrder, findOrder_upsert, ite_eq_left hid'.symm]
  · intro y hy
    have hy' : y ≠ o'.id := by simpa [hid'] using hy
    rw [hbook, orders_dequeue, writeOrder, findOrder_upsert, ite_eq_right hy']
  · rw [hbook]; cases o.side <;> rfl
  · rw [hbook]; cases o.side <;> rfl

theorem law9_holds : law9 := by
  intro book hreach x
  have hinv := reachable_inv hreach
  refine ⟨rfl, ?_⟩
  cases hfind : findOrder book.orders x with
  | none =>
    simp [apply, cancelOrder, hfind]
  | some o =>
    cases hact : o.status.isActive with
    | false =>
      simp [apply, cancelOrder, hfind, hact]
    | true =>
      simpa [apply, hfind, hact] using cancel_keeps_others hinv hfind hact

end Nanobook
