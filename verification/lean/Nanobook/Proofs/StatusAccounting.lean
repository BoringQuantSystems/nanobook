/-
A submit record matches its status.
-/

import Nanobook.Proofs.Conservation

namespace Nanobook

theorem stopIncoming_zero (o : Order) :
    (stopIncoming o).remaining = 0 ∧ (stopIncoming o).filled = o.filled := by
  simp [stopIncoming]

theorem step_stop_incoming (book : Book) (incoming : Order) (policy : StpPolicy)
    (h : (step book incoming policy false).stp = true) :
    (step book incoming policy false).incoming = stopIncoming incoming ∧
    (step book incoming policy false).go = false := by
  cases hsucc : incoming.remaining
  case zero =>
    have hs : step book incoming policy false =
        { book, incoming, stp := false, trade := none, go := false } := by
      simp [step, hsucc]
    simp [hs] at h
  case succ n =>
    let opp := incoming.side.opposite
    cases hbest : bestPrice (levelsOf book opp)
    case none =>
      have hs : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc, hbest, opp]
      simp [hs] at h
    case some px =>
      cases hcross : pricesCross incoming.side incoming.price px
      case false =>
        have hs : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hsucc, hbest, hcross, opp]
        simp [hs] at h
      case true =>
        cases hfront : frontAt (levelsOf book opp) px
        case none =>
          have hs : step book incoming policy false =
              { book := dropLevel book opp px, incoming, stp := false, trade := none,
                go := true } := by
            simp [step, hsucc, hbest, hcross, hfront, opp]
          simp [hs] at h
        case some rid =>
          cases hfind : findOrder book.orders rid
          case none =>
            have hs : step book incoming policy false =
                { book := dequeue book opp px rid, incoming, stp := false, trade := none,
                  go := true } := by
              simp [step, hsucc, hbest, hcross, hfront, hfind, opp]
            simp [hs] at h
          case some resting =>
            cases hdead : (!resting.status.isActive || resting.remaining == 0)
            case true =>
              have hs : step book incoming policy false =
                  { book := dequeue book opp px rid, incoming, stp := false, trade := none,
                    go := true } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hdead, opp]
              simp [hs] at h
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
                have hconf : stpConflict incoming resting .off = false :=
                  stpConflict_off incoming resting
                let qty := min incoming.remaining resting.remaining
                let emitted := emitTrade book px qty incoming.id rid incoming.side
                let incoming' := fill incoming qty
                let resting' := fill resting qty
                let book1 := writeOrder emitted.1 resting'
                let book2 := if resting'.remaining == 0 then dequeue book1 opp px rid else book1
                have hs : step book incoming .off false =
                    { book := book2, incoming := incoming', stp := false,
                      trade := some emitted.2, go := true } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty,
                    emitted, incoming', resting', book1, book2, opp]
                simp [hs] at h
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
                  let qty := min incoming.remaining resting.remaining
                  let emitted := emitTrade book px qty incoming.id rid incoming.side
                  let incoming' := fill incoming qty
                  let resting' := fill resting qty
                  let book1 := writeOrder emitted.1 resting'
                  let book2 :=
                    if resting'.remaining == 0 then dequeue book1 opp px rid else book1
                  have hs : step book incoming .cancelNewest false =
                      { book := book2, incoming := incoming', stp := false,
                        trade := some emitted.2, go := true } := by
                    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty,
                      emitted, incoming', resting', book1, book2, opp]
                  simp [hs] at h
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
                  simp [hs] at h
                case false =>
                  let qty := min incoming.remaining resting.remaining
                  let emitted := emitTrade book px qty incoming.id rid incoming.side
                  let incoming' := fill incoming qty
                  let resting' := fill resting qty
                  let book1 := writeOrder emitted.1 resting'
                  let book2 :=
                    if resting'.remaining == 0 then dequeue book1 opp px rid else book1
                  have hs : step book incoming .cancelOldest false =
                      { book := book2, incoming := incoming', stp := false,
                        trade := some emitted.2, go := true } := by
                    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty,
                      emitted, incoming', resting', book1, book2, opp]
                  simp [hs] at h
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
                    simp [hs] at h
                case false =>
                  let qty := min incoming.remaining resting.remaining
                  let emitted := emitTrade book px qty incoming.id rid incoming.side
                  let incoming' := fill incoming qty
                  let resting' := fill resting qty
                  let book1 := writeOrder emitted.1 resting'
                  let book2 :=
                    if resting'.remaining == 0 then dequeue book1 opp px rid else book1
                  have hs : step book incoming .decrementAndCancel false =
                      { book := book2, incoming := incoming', stp := false,
                        trade := some emitted.2, go := true } := by
                    simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, qty,
                      emitted, incoming', resting', book1, book2, opp]
                  simp [hs] at h

theorem matchLoop_stop_lt (fuel : Nat) (policy : StpPolicy)
    {src : Book} {budget : Nat} {side : Side} {limit : Int}
    {book : Book} {incoming : Order} {trades : List Trade}
    (h : MatchAcc src budget side limit book incoming trades false) :
    (matchLoop fuel book incoming policy trades false).2.2.2 = true →
      (matchLoop fuel book incoming policy trades false).2.1.remaining = 0 ∧
      (matchLoop fuel book incoming policy trades false).2.1.filled < budget := by
  induction fuel generalizing book incoming trades with
  | zero =>
    intro hstp
    simp [matchLoop] at hstp
  | succ n ih =>
    have hstep := step_acc policy h
    cases hgo : (step book incoming policy false).go
    · have hret :
          matchLoop (n + 1) book incoming policy trades false =
            ((step book incoming policy false).book,
              (step book incoming policy false).incoming,
              (match (step book incoming policy false).trade with
                | none => trades
                | some t => t :: trades).reverse,
              (step book incoming policy false).stp) := by
        simp [matchLoop, hgo]
        rfl
      rw [hret]
      simp
      intro hstp
      have ⟨hin, _⟩ := step_stop_incoming book incoming policy hstp
      have hz := stopIncoming_zero incoming
      have hlive := h.live rfl
      have hne : incoming.remaining ≠ 0 := by
        intro hz0
        have hs : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hz0]
        have hfalse : (step book incoming policy false).stp = false := by simp [hs]
        simp [hfalse] at hstp
      have hpos : 0 < incoming.remaining := Nat.pos_of_ne_zero hne
      have hfill : (step book incoming policy false).incoming.filled = incoming.filled := by
        simpa [hin] using hz.2
      refine ⟨?_, ?_⟩
      · simpa [hin] using hz.1
      · omega
    · have hstpF : (step book incoming policy false).stp = false := by
        cases hst : (step book incoming policy false).stp
        · rfl
        · have hgo' := (step_stop_incoming book incoming policy (by simp [hst])).2
          simp [hgo] at hgo'
      have hret :
          matchLoop (n + 1) book incoming policy trades false =
            matchLoop n (step book incoming policy false).book
              (step book incoming policy false).incoming policy
              (match (step book incoming policy false).trade with
                | none => trades
                | some t => t :: trades)
              (step book incoming policy false).stp := by
        simp [matchLoop, hgo]
        rfl
      rw [hret, hstpF]
      have hstepF : MatchAcc src budget side limit
          (step book incoming policy false).book
          (step book incoming policy false).incoming
          (match (step book incoming policy false).trade with
            | none => trades
            | some t => t :: trades)
          false := by
        rw [hstpF] at hstep
        exact hstep
      exact ih hstepF

theorem settle_stp_record (book : Book) (o : Order) :
    findOrder (settle book o true).orders o.id =
      some { o with status := .cancelled } := by
  simp [settle, writeOrder, findOrder_upsert]

theorem settle_filled_record (book : Book) (o : Order)
    (hzero : (o.remaining == 0) = true) :
    findOrder (settle book o false).orders o.id =
      some { o with status := .filled } := by
  simp [settle, hzero, writeOrder, findOrder_upsert]

theorem settle_gtc_record (book : Book) (o : Order)
    (hzero : (o.remaining == 0) = false) (htif : (o.timeInForce == .gtc) = true) :
    findOrder (settle book o false).orders o.id =
      some { o with status := if o.filled > 0 then .partiallyFilled else .new } := by
  simp [settle, hzero, htif, orders_enqueue, writeOrder, findOrder_upsert]

theorem settle_ioc_record (book : Book) (o : Order)
    (hzero : (o.remaining == 0) = false) (htif : (o.timeInForce == .gtc) = false) :
    findOrder (settle book o false).orders o.id =
      some { o with remaining := 0, status := .cancelled } := by
  simp [settle, hzero, htif, writeOrder, findOrder_upsert]

theorem gtc_rest_status (filled : Nat) :
    let s := if filled > 0 then OrderStatus.partiallyFilled else OrderStatus.new
    s.isActive = true ∧ s ≠ .filled ∧ s ≠ .cancelled := by
  dsimp
  split <;> simp [OrderStatus.isActive]

theorem status_filled (o : Order) (quantity : Nat)
    (hs : o.status = .filled) (hf : o.filled = quantity) (hr : o.remaining = 0) :
    (o.status = .filled → o.filled = quantity ∧ o.remaining = 0) ∧
    (o.status.isActive = true → o.filled + o.remaining = quantity ∧ 0 < o.remaining) ∧
    (o.status = .cancelled → o.remaining = 0 ∧ o.filled < quantity) := by
  refine ⟨fun _ => ⟨hf, hr⟩, ?_, ?_⟩
  · intro hact
    simp [hs, OrderStatus.isActive] at hact
  · intro hc
    simp [hs] at hc

theorem status_cancelled (o : Order) (quantity : Nat)
    (hs : o.status = .cancelled) (hr : o.remaining = 0) (hlt : o.filled < quantity) :
    (o.status = .filled → o.filled = quantity ∧ o.remaining = 0) ∧
    (o.status.isActive = true → o.filled + o.remaining = quantity ∧ 0 < o.remaining) ∧
    (o.status = .cancelled → o.remaining = 0 ∧ o.filled < quantity) := by
  refine ⟨?_, ?_, fun _ => ⟨hr, hlt⟩⟩
  · intro hf
    simp [hs] at hf
  · intro hact
    simp [hs, OrderStatus.isActive] at hact

theorem status_resting (o : Order) (quantity : Nat)
    (hneF : o.status ≠ .filled) (hneC : o.status ≠ .cancelled)
    (hsum : o.filled + o.remaining = quantity) (hpos : 0 < o.remaining) :
    (o.status = .filled → o.filled = quantity ∧ o.remaining = 0) ∧
    (o.status.isActive = true → o.filled + o.remaining = quantity ∧ 0 < o.remaining) ∧
    (o.status = .cancelled → o.remaining = 0 ∧ o.filled < quantity) := by
  refine ⟨fun hf => (hneF hf).elim, fun _ => ⟨hsum, hpos⟩, fun hc => (hneC hc).elim⟩

theorem submit_record (h : BookInv book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) (policy : StpPolicy) :
    match findOrder (submit book side price quantity tif owner policy).1.orders
        book.nextOrderId with
    | none => (submit book side price quantity tif owner policy).2 = []
    | some o =>
        (o.status = .filled → o.filled = quantity ∧ o.remaining = 0) ∧
        (o.status.isActive = true → o.filled + o.remaining = quantity ∧ 0 < o.remaining) ∧
        (o.status = .cancelled → o.remaining = 0 ∧ o.filled < quantity) := by
  by_cases hrej :
      (tif == .fok && decide (available book side price quantity owner policy < quantity)) = true
  · have hsub : submit book side price quantity tif owner policy =
        ((alloc book side price quantity tif owner).1, []) := by
      simp [submit, hrej]
    rw [hsub]
    have halloc := bookInv_alloc h side price quantity tif owner
    have hnone : findOrder (alloc book side price quantity tif owner).1.orders
        book.nextOrderId = none := by
      apply findOrder_below
      intro p hp
      have hlt := halloc.2.2.1 p hp
      simpa [halloc.2.1] using hlt
    simp [hnone]
  · have hfalse : (tif == .fok &&
        decide (available book side price quantity owner policy < quantity)) = false := by
      cases hcond :
          (tif == .fok && decide (available book side price quantity owner policy < quantity))
      · rfl
      · exact (hrej hcond).elim
    let out := alloc book side price quantity tif owner
    let looped := matchLoop (matchFuel out.1 out.2) out.1 out.2 policy [] false
    have hsub : submit book side price quantity tif owner policy =
        (settle looped.1 looped.2.1 looped.2.2.2, looped.2.2.1) := by
      simp [submit, hfalse, out, looped]
    rw [hsub]
    change match findOrder (settle looped.1 looped.2.1 looped.2.2.2).orders
        book.nextOrderId with
      | none => looped.2.2.1 = []
      | some o =>
          (o.status = .filled → o.filled = quantity ∧ o.remaining = 0) ∧
          (o.status.isActive = true → o.filled + o.remaining = quantity ∧ 0 < o.remaining) ∧
          (o.status = .cancelled → o.remaining = 0 ∧ o.filled < quantity)
    have hacc : MatchAcc book quantity side price out.1 out.2 [] false := by
      simpa [out] using alloc_acc h side price quantity tif owner
    have hloop := matchLoop_acc (matchFuel out.1 out.2) policy hacc
    have hid : looped.2.1.id = book.nextOrderId := by
      simpa [out] using hloop.ident
    have hstop : looped.2.2.2 = true →
        looped.2.1.remaining = 0 ∧ looped.2.1.filled < quantity := by
      simpa [out, looped] using matchLoop_stop_lt (matchFuel out.1 out.2) policy hacc
    cases hstp : looped.2.2.2
    case true =>
      have ⟨hrem0, hlt⟩ := hstop hstp
      let stored : Order := { looped.2.1 with status := .cancelled }
      have hfind : findOrder (settle looped.1 looped.2.1 true).orders book.nextOrderId =
          some stored := by
        simpa [hid, stored] using settle_stp_record looped.1 looped.2.1
      rw [hfind]
      exact status_cancelled stored quantity rfl (by simp [stored, hrem0])
        (by simpa [stored] using hlt)
    case false =>
      have hlive : looped.2.1.filled + looped.2.1.remaining = quantity := hloop.live hstp
      cases hzero : (looped.2.1.remaining == 0)
      case true =>
        have hrem0 : looped.2.1.remaining = 0 := by simpa using hzero
        have hfill : looped.2.1.filled = quantity := by omega
        let stored : Order := { looped.2.1 with status := .filled }
        have hfind : findOrder (settle looped.1 looped.2.1 false).orders book.nextOrderId =
            some stored := by
          simpa [hid, stored] using settle_filled_record looped.1 looped.2.1 hzero
        rw [hfind]
        exact status_filled stored quantity rfl (by simp [stored, hfill])
          (by simp [stored, hrem0])
      case false =>
        have hne : looped.2.1.remaining ≠ 0 := by simpa using hzero
        have hpos : 0 < looped.2.1.remaining := Nat.pos_of_ne_zero hne
        have hlt : looped.2.1.filled < quantity := by omega
        cases htif : (looped.2.1.timeInForce == .gtc)
        case true =>
          let stored : Order :=
            { looped.2.1 with
              status := if looped.2.1.filled > 0 then .partiallyFilled else .new }
          have hfind : findOrder (settle looped.1 looped.2.1 false).orders book.nextOrderId =
              some stored := by
            simpa [hid, stored] using settle_gtc_record looped.1 looped.2.1 hzero htif
          have hs := gtc_rest_status looped.2.1.filled
          rw [hfind]
          exact status_resting stored quantity
            (by simpa [stored] using hs.2.1) (by simpa [stored] using hs.2.2)
            (by simpa [stored] using hlive) (by simpa [stored] using hpos)
        case false =>
          let stored : Order := { looped.2.1 with remaining := 0, status := .cancelled }
          have hfind : findOrder (settle looped.1 looped.2.1 false).orders book.nextOrderId =
              some stored := by
            simpa [hid, stored] using settle_ioc_record looped.1 looped.2.1 hzero htif
          rw [hfind]
          exact status_cancelled stored quantity rfl rfl (by simpa [stored] using hlt)

theorem apply_record (h : BookInv book) (event : Event) (view : SubmitView)
    (hview : eventSubmit event = some view) :
    match findOrder (apply book event).1.orders book.nextOrderId with
    | none => (apply book event).2 = []
    | some o =>
        (o.status = .filled → o.filled = view.quantity ∧ o.remaining = 0) ∧
        (o.status.isActive = true → o.filled + o.remaining = view.quantity ∧ 0 < o.remaining) ∧
        (o.status = .cancelled → o.remaining = 0 ∧ o.filled < view.quantity) := by
  cases event with
  | cancel _ =>
    simp [eventSubmit] at hview
  | submitLimit side price quantity tif owner policy =>
    simp only [eventSubmit] at hview
    injection hview with hview
    cases hview
    simpa [apply] using submit_record h side price quantity tif owner policy
  | submitMarket side quantity owner policy =>
    cases side with
    | buy =>
      simp only [eventSubmit] at hview
      injection hview with hview
      cases hview
      simpa [apply] using submit_record h .buy i64Max quantity .ioc owner policy
    | sell =>
      simp only [eventSubmit] at hview
      injection hview with hview
      cases hview
      simpa [apply] using submit_record h .sell i64Min quantity .ioc owner policy

theorem law11_holds : law11 := by
  intro book hReach event view book' trades hview happ
  have hf := apply_record (reachable_inv hReach) event view hview
  rw [happ] at hf
  exact hf

end Nanobook
