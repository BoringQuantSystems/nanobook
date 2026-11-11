/-
Self-trade prevention for one match step.
-/

import Nanobook.Proofs.WellFormed

namespace Nanobook

theorem owners_clear {incoming resting : Order} {policy : StpPolicy}
    (hpol : policy ≠ .off) (hconf : stpConflict incoming resting policy = false) :
    ownersConflict incoming.owner resting.owner = false := by
  cases hi : incoming.owner <;> cases hr : resting.owner
  · simp [ownersConflict]
  · simp [ownersConflict]
  · simp [ownersConflict]
  · cases policy
    · exact absurd rfl hpol
    · simp [ownersConflict, stpConflict, hi, hr] at hconf ⊢
      exact hconf
    · simp [ownersConflict, stpConflict, hi, hr] at hconf ⊢
      exact hconf
    · simp [ownersConflict, stpConflict, hi, hr] at hconf ⊢
      exact hconf

theorem stp_on {incoming resting : Order} {policy : StpPolicy}
    (hown : ownersConflict incoming.owner resting.owner = true) (hpol : policy ≠ .off) :
    stpConflict incoming resting policy = true := by
  cases hi : incoming.owner <;> cases hr : resting.owner <;>
    simp [ownersConflict, hi, hr] at hown
  cases policy
  · exact absurd rfl hpol
  · simp [stpConflict, hi, hr, hown]
  · simp [stpConflict, hi, hr, hown]
  · simp [stpConflict, hi, hr, hown]

theorem step_fill_trade (book : Book) (incoming resting : Order) (policy : StpPolicy)
    {px : Int} {rid n : Nat}
    (hsucc : incoming.remaining = Nat.succ n)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (hact : resting.status.isActive = true) (hne : resting.remaining ≠ 0)
    (hconf : stpConflict incoming resting policy = false) :
    ∃ t, (step book incoming policy false).trade = some t ∧
      t.price = px ∧ t.passiveId = rid ∧
      t.quantity = min incoming.remaining resting.remaining ∧
      (step book incoming policy false).go = true ∧
      (step book incoming policy false).stp = false := by
  let opp := incoming.side.opposite
  let qty := min incoming.remaining resting.remaining
  let emitted := emitTrade book px qty incoming.id rid incoming.side
  let incoming' := fill incoming qty
  let resting' := fill resting qty
  let book1 := writeOrder emitted.1 resting'
  let book2 := if resting'.remaining == 0 then dequeue book1 opp px rid else book1
  have hs : step book incoming policy false =
      { book := book2, incoming := incoming', stp := false, trade := some emitted.2,
        go := true } := by
    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty, emitted,
      incoming', resting', book1, book2, opp]
  refine ⟨emitted.2, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · simp [hs]
  · simp [emitted, emitTrade]
  · simp [emitted, emitTrade]
  · simp [emitted, emitTrade, qty]
  · simp [hs]
  · simp [hs]

theorem trade_owners (book : Book) (incoming : Order) (policy : StpPolicy) (stp : Bool) :
    match (step book incoming policy stp).trade with
    | none => True
    | some t =>
      policy = .off ∨
        ∀ resting, findOrder book.orders t.passiveId = some resting →
          ownersConflict incoming.owner resting.owner = false := by
  cases stp
  case true =>
    have hs : step book incoming policy true =
        { book, incoming, stp := true, trade := none, go := false } := by
      simp [step]
    simp [hs]
  case false =>
    cases hsucc : incoming.remaining
    case zero =>
      have hs : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc]
      simp [hs]
    case succ n =>
      let opp := incoming.side.opposite
      cases hbest : bestPrice (levelsOf book opp)
      case none =>
        have hs : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hsucc, hbest, opp]
        simp [hs]
      case some px =>
        cases hcross : pricesCross incoming.side incoming.price px
        case false =>
          have hs : step book incoming policy false =
              { book, incoming, stp := false, trade := none, go := false } := by
            simp [step, hsucc, hbest, hcross, opp]
          simp [hs]
        case true =>
          cases hfront : frontAt (levelsOf book opp) px
          case none =>
            have hs : step book incoming policy false =
                { book := dropLevel book opp px, incoming, stp := false, trade := none,
                  go := true } := by
              simp [step, hsucc, hbest, hcross, hfront, opp]
            simp [hs]
          case some rid =>
            cases hfind : findOrder book.orders rid
            case none =>
              have hs : step book incoming policy false =
                  { book := dequeue book opp px rid, incoming, stp := false, trade := none,
                    go := true } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, opp]
              simp [hs]
            case some resting =>
              cases hdead : (!resting.status.isActive || resting.remaining == 0)
              case true =>
                have hs : step book incoming policy false =
                    { book := dequeue book opp px rid, incoming, stp := false, trade := none,
                      go := true } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hdead, opp]
                simp [hs]
              case false =>
                cases hact : resting.status.isActive <;>
                  cases hz : (resting.remaining == 0) <;>
                  simp [hact, hz] at hdead
                have hne : resting.remaining ≠ 0 := by
                  intro heq
                  rw [heq] at hz
                  simp at hz
                cases policy
                case off =>
                  rcases step_fill_trade book incoming resting .off hsucc hbest hcross hfront
                      hfind hact hne (stpConflict_off incoming resting) with
                    ⟨t, ht, _, _, _, _, _⟩
                  simp [ht]
                case cancelNewest =>
                  cases hconf : stpConflict incoming resting .cancelNewest
                  case true =>
                    have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
                    have hs : step book incoming .cancelNewest false =
                        { book, incoming := stopIncoming incoming, stp := true, trade := none,
                          go := false } := by
                      simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq,
                        opp]
                    simp [hs]
                  case false =>
                    rcases step_fill_trade book incoming resting .cancelNewest hsucc hbest hcross
                        hfront hfind hact hne hconf with
                      ⟨t, ht, _, hid, _, _, _⟩
                    simp [ht]
                    intro resting' hf
                    have hr : resting' = resting := by
                      have hf' : some resting' = some resting := by rw [← hf, hid, hfind]
                      injection hf' with hr
                    simpa [hr] using
                      owners_clear (by intro h; cases h) hconf
                case cancelOldest =>
                  cases hconf : stpConflict incoming resting .cancelOldest
                  case true =>
                    have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
                    have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
                    have hs : step book incoming .cancelOldest false =
                        { book := cancelOrder book rid, incoming, stp := false, trade := none,
                          go := true } := by
                      simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                        hsame, opp]
                    simp [hs]
                  case false =>
                    rcases step_fill_trade book incoming resting .cancelOldest hsucc hbest hcross
                        hfront hfind hact hne hconf with
                      ⟨t, ht, _, hid, _, _, _⟩
                    simp [ht]
                    intro resting' hf
                    have hr : resting' = resting := by
                      have hf' : some resting' = some resting := by rw [← hf, hid, hfind]
                      injection hf' with hr
                    simpa [hr] using
                      owners_clear (by intro h; cases h) hconf
                case decrementAndCancel =>
                  cases hconf : stpConflict incoming resting .decrementAndCancel
                  case true =>
                    have hnotNew :
                        (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
                    have hnotOld :
                        (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
                    have hdec :
                        (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
                    by_cases hlt : incoming.remaining < resting.remaining
                    · have hltN : n + 1 < resting.remaining := by simpa [hsucc] using hlt
                      have hs : step book incoming .decrementAndCancel false =
                          { book, incoming := stopIncoming incoming, stp := true, trade := none,
                            go := false } := by
                        simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf,
                          hnotNew, hnotOld, hdec, hltN, opp]
                      simp [hs]
                    · have hnotN : ¬ n + 1 < resting.remaining := by simpa [hsucc] using hlt
                      have hs : step book incoming .decrementAndCancel false =
                          { book := cancelOrder book rid, incoming, stp := false, trade := none,
                            go := true } := by
                        simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf,
                          hnotNew, hnotOld, hdec, hnotN, opp]
                      simp [hs]
                  case false =>
                    rcases step_fill_trade book incoming resting .decrementAndCancel hsucc hbest
                        hcross hfront hfind hact hne hconf with
                      ⟨t, ht, _, hid, _, _, _⟩
                    simp [ht]
                    intro resting' hf
                    have hr : resting' = resting := by
                      have hf' : some resting' = some resting := by rw [← hf, hid, hfind]
                      injection hf' with hr
                    simpa [hr] using
                      owners_clear (by intro h; cases h) hconf

theorem policy_effect (book : Book) (incoming : Order) (policy : StpPolicy)
    {rid : Nat} {resting : Order} {px : Int}
    (hrem : 0 < incoming.remaining)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (hact : resting.status.isActive = true) (hpos : 0 < resting.remaining)
    (hown : ownersConflict incoming.owner resting.owner = true) :
    let s := step book incoming policy false
    match policy with
    | .off =>
      ∃ t, s.trade = some t ∧ t.price = px ∧ t.passiveId = rid ∧
        t.quantity = min incoming.remaining resting.remaining ∧
        s.go = true ∧ s.stp = false
    | .cancelNewest =>
      s.book = book ∧ s.incoming = stopIncoming incoming ∧ s.stp = true ∧
        s.trade = none ∧ s.go = false
    | .cancelOldest =>
      s.book = cancelOrder book rid ∧ s.incoming = incoming ∧ s.stp = false ∧
        s.trade = none ∧ s.go = true
    | .decrementAndCancel =>
      if incoming.remaining < resting.remaining then
        s.book = book ∧ s.incoming = stopIncoming incoming ∧ s.stp = true ∧
          s.trade = none ∧ s.go = false
      else
        s.book = cancelOrder book rid ∧ s.incoming = incoming ∧ s.stp = false ∧
          s.trade = none ∧ s.go = true := by
  cases hsucc : incoming.remaining
  case zero =>
    omega
  case succ n =>
    have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hpos
    let opp := incoming.side.opposite
    cases policy
    case off =>
      rcases step_fill_trade book incoming resting .off hsucc hbest hcross hfront hfind hact hne
          (stpConflict_off incoming resting) with
        ⟨t, ht, hp, hid, hq, hg, hstp⟩
      refine ⟨t, ?_, ?_, ?_, ?_, ?_, ?_⟩
      · simpa [hsucc] using ht
      · simpa [hsucc] using hp
      · simpa [hsucc] using hid
      · simpa [hsucc] using hq
      · simpa [hsucc] using hg
      · simpa [hsucc] using hstp
    case cancelNewest =>
      have hconf := stp_on (policy := .cancelNewest) hown (by intro h; cases h)
      have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
      have hs : step book incoming .cancelNewest false =
          { book, incoming := stopIncoming incoming, stp := true, trade := none, go := false } := by
        simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq]
      simp [hs]
    case cancelOldest =>
      have hconf := stp_on (policy := .cancelOldest) hown (by intro h; cases h)
      have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
      have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
      have hs : step book incoming .cancelOldest false =
          { book := cancelOrder book rid, incoming, stp := false, trade := none, go := true } := by
        simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hsame]
      simp [hs]
    case decrementAndCancel =>
      have hconf := stp_on (policy := .decrementAndCancel) hown (by intro h; cases h)
      have hnotNew : (StpPolicy.decrementAndCancel == .cancelNewest) = false := by decide
      have hnotOld : (StpPolicy.decrementAndCancel == .cancelOldest) = false := by decide
      have hdec : (StpPolicy.decrementAndCancel == .decrementAndCancel) = true := by decide
      by_cases hlt : n + 1 < resting.remaining
      · have hs : step book incoming .decrementAndCancel false =
            { book, incoming := stopIncoming incoming, stp := true, trade := none,
              go := false } := by
          simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hnotOld,
            hdec, hlt]
        simp [hlt, hs]
      · have hs : step book incoming .decrementAndCancel false =
            { book := cancelOrder book rid, incoming, stp := false, trade := none,
              go := true } := by
          simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hnotOld,
            hdec, hlt]
        simp [hlt, hs]

theorem law8_holds : law8 := by
  intro book incoming policy stp
  refine ⟨trade_owners book incoming policy stp, ?_⟩
  intro rid resting px _hstp hrem hbest hcross hfront hfind hact hpos hown
  exact policy_effect book incoming policy hrem hbest hcross hfront hfind hact hpos hown

end Nanobook
