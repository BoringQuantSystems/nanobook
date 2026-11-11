import Nanobook.Proofs.Conservation

namespace Nanobook

theorem law5_holds : law5 := by
  intro book hReach event view book' trades hview happ t ht
  have hf := apply_submit_facts (reachable_inv hReach) event view hview
  have hf' : SubmitFacts book view.side view.price view.quantity book' trades := by
    simpa [happ] using hf
  refine ⟨hf'.atLevel t ht, ?_⟩
  cases hside : view.side <;> simpa [withinLimit, hside] using hf'.worse t ht

end Nanobook
