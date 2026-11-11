/-
A reachable book never shows a crossed quote.
The best bid stays below the best ask.
-/

import Nanobook.Proofs.Fuel

namespace Nanobook

open List

/-- Best bid is strictly below the best ask. -/
def uncrossed (book : Book) : Prop :=
  ∀ bid ask,
    bestPrice book.bids = some bid →
    bestPrice book.asks = some ask →
    bid < ask

/-- The sweep stopped, or the opposite best does not cross. -/
def matchedDone (book : Book) (incoming : Order) (stp : Bool) : Prop :=
  stp = true ∨ incoming.remaining = 0 ∨
    ∀ px, bestPrice (levelsOf book incoming.side.opposite) = some px →
      pricesCross incoming.side incoming.price px = false

theorem not_cross_buy {limit px : Int} (h : pricesCross .buy limit px = false) : limit < px := by
  simp only [pricesCross] at h
  have hnot : ¬ (limit ≥ px) := of_decide_eq_false h
  omega

theorem not_cross_sell {limit px : Int} (h : pricesCross .sell limit px = false) : px < limit := by
  simp only [pricesCross] at h
  have hnot : ¬ (limit ≤ px) := of_decide_eq_false h
  omega

theorem best_insertBid (levels : List Level) (price : Int) (id : Nat) :
    bestPrice (insertBid levels price id) =
      match bestPrice levels with
      | none => some price
      | some b => some (max b price) := by
  cases levels with
  | nil => simp [insertBid, bestPrice]
  | cons lvl tail =>
    have hrhs : bestPrice (lvl :: tail) = some lvl.price := by simp [bestPrice]
    rw [hrhs]
    simp only [insertBid]
    split
    · rename_i hgt
      simp [bestPrice]
      exact (Int.max_eq_right (Int.le_of_lt hgt)).symm
    · split
      · rename_i heq
        have hp : price = lvl.price := by simpa using heq
        simp [bestPrice, hp]
      · rename_i hnot hne
        have hne' : price ≠ lvl.price := by simpa using hne
        have hlt : price < lvl.price := by omega
        simp [bestPrice]
        exact (Int.max_eq_left (Int.le_of_lt hlt)).symm

theorem best_insertAsk (levels : List Level) (price : Int) (id : Nat) :
    bestPrice (insertAsk levels price id) =
      match bestPrice levels with
      | none => some price
      | some a => some (min a price) := by
  cases levels with
  | nil => simp [insertAsk, bestPrice]
  | cons lvl tail =>
    have hrhs : bestPrice (lvl :: tail) = some lvl.price := by simp [bestPrice]
    rw [hrhs]
    simp only [insertAsk]
    split
    · rename_i hlt
      simp [bestPrice]
      exact (Int.min_eq_right (Int.le_of_lt hlt)).symm
    · split
      · rename_i heq
        have hp : price = lvl.price := by simpa using heq
        simp [bestPrice, hp]
      · rename_i hnot hne
        have hne' : price ≠ lvl.price := by simpa using hne
        have hgt : lvl.price < price := by omega
        simp [bestPrice]
        exact (Int.min_eq_left (Int.le_of_lt hgt)).symm

theorem uncrossed_enqueue (hu : uncrossed book) (o : Order)
    (hsafe : ∀ px, bestPrice (levelsOf book o.side.opposite) = some px →
      pricesCross o.side o.price px = false) :
    uncrossed (enqueue book o) := by
  intro bid ask hbid hask
  cases hside : o.side with
  | buy =>
    have hbids : (enqueue book o).bids = insertBid book.bids o.price o.id := by
      simp [enqueue, hside]
    have hasks : (enqueue book o).asks = book.asks := by
      simp [enqueue, hside]
    rw [hbids] at hbid
    rw [hasks] at hask
    have hpx : o.price < ask := by
      rw [hside] at hsafe
      exact not_cross_buy (hsafe ask hask)
    rw [best_insertBid] at hbid
    cases hb0 : bestPrice book.bids with
    | none =>
      rw [hb0] at hbid
      injection hbid with heq
      omega
    | some b =>
      rw [hb0] at hbid
      injection hbid with heq
      have hbb : b < ask := hu b ask hb0 hask
      have hmax : max b o.price < ask := (Int.max_lt).mpr ⟨hbb, hpx⟩
      omega
  | sell =>
    have hbids : (enqueue book o).bids = book.bids := by
      simp [enqueue, hside]
    have hasks : (enqueue book o).asks = insertAsk book.asks o.price o.id := by
      simp [enqueue, hside]
    rw [hbids] at hbid
    rw [hasks] at hask
    have hpx : bid < o.price := by
      rw [hside] at hsafe
      exact not_cross_sell (hsafe bid hbid)
    rw [best_insertAsk] at hask
    cases ha0 : bestPrice book.asks with
    | none =>
      rw [ha0] at hask
      injection hask with heq
      omega
    | some a =>
      rw [ha0] at hask
      injection hask with heq
      have haa : bid < a := hu bid a hbid ha0
      have hmin : bid < min a o.price := by
        rw [Int.min_def]
        split <;> omega
      omega

theorem best_removeAt_desc {levels : List Level}
    (h : levels.Pairwise (fun a b => a.price > b.price))
    {price : Int} {id : Nat} {b' : Int}
    (hb : bestPrice (removeAt levels price id) = some b') :
    ∃ b, bestPrice levels = some b ∧ b' ≤ b := by
  cases levels with
  | nil => simp [removeAt, bestPrice] at hb
  | cons lvl tail =>
    have hcons := (pairwise_cons.mp h).1
    cases hp : (lvl.price == price)
    · have hstep : removeAt (lvl :: tail) price id = lvl :: removeAt tail price id := by
        simp [removeAt, hp]
      rw [hstep, bestPrice] at hb
      injection hb with hb'
      subst hb'
      exact ⟨lvl.price, rfl, Int.le_refl _⟩
    · cases hempty : (removeId lvl.orders id).isEmpty
      · have hstep : removeAt (lvl :: tail) price id =
            { lvl with orders := removeId lvl.orders id } :: tail := by
          simp [removeAt, hp, hempty]
        rw [hstep, bestPrice] at hb
        injection hb with hb'
        subst hb'
        exact ⟨lvl.price, rfl, Int.le_refl _⟩
      · have hstep : removeAt (lvl :: tail) price id = tail := by
          simp [removeAt, hp, hempty]
        rw [hstep] at hb
        cases tail with
        | nil => simp [bestPrice] at hb
        | cons nxt rest =>
          rw [bestPrice] at hb
          injection hb with hb'
          have hgt : nxt.price < lvl.price := hcons nxt mem_cons_self
          subst hb'
          exact ⟨lvl.price, rfl, Int.le_of_lt hgt⟩

theorem best_removeAt_asc {levels : List Level}
    (h : levels.Pairwise (fun a b => a.price < b.price))
    {price : Int} {id : Nat} {a' : Int}
    (ha : bestPrice (removeAt levels price id) = some a') :
    ∃ a, bestPrice levels = some a ∧ a ≤ a' := by
  cases levels with
  | nil => simp [removeAt, bestPrice] at ha
  | cons lvl tail =>
    have hcons := (pairwise_cons.mp h).1
    cases hp : (lvl.price == price)
    · have hstep : removeAt (lvl :: tail) price id = lvl :: removeAt tail price id := by
        simp [removeAt, hp]
      rw [hstep, bestPrice] at ha
      injection ha with ha'
      subst ha'
      exact ⟨lvl.price, rfl, Int.le_refl _⟩
    · cases hempty : (removeId lvl.orders id).isEmpty
      · have hstep : removeAt (lvl :: tail) price id =
            { lvl with orders := removeId lvl.orders id } :: tail := by
          simp [removeAt, hp, hempty]
        rw [hstep, bestPrice] at ha
        injection ha with ha'
        subst ha'
        exact ⟨lvl.price, rfl, Int.le_refl _⟩
      · have hstep : removeAt (lvl :: tail) price id = tail := by
          simp [removeAt, hp, hempty]
        rw [hstep] at ha
        cases tail with
        | nil => simp [bestPrice] at ha
        | cons nxt rest =>
          rw [bestPrice] at ha
          injection ha with ha'
          have hlt : lvl.price < nxt.price := hcons nxt mem_cons_self
          subst ha'
          exact ⟨lvl.price, rfl, Int.le_of_lt hlt⟩

theorem uncrossed_dequeue (hu : uncrossed book)
    (hbid : bestFirst .buy book.bids) (hask : bestFirst .sell book.asks)
    (side : Side) (px : Int) (id : Nat) :
    uncrossed (dequeue book side px id) := by
  intro bid ask hb ha
  cases side with
  | buy =>
    have hbids : (dequeue book .buy px id).bids = removeAt book.bids px id := by
      simp [dequeue, setLevels, levelsOf]
    have hasks : (dequeue book .buy px id).asks = book.asks := by
      simp [dequeue, setLevels, levelsOf]
    rw [hbids] at hb
    rw [hasks] at ha
    rcases best_removeAt_desc (by simpa [bestFirst] using hbid) hb with ⟨b, hb0, hle⟩
    have hbb : b < ask := hu b ask hb0 ha
    omega
  | sell =>
    have hbids : (dequeue book .sell px id).bids = book.bids := by
      simp [dequeue, setLevels, levelsOf]
    have hasks : (dequeue book .sell px id).asks = removeAt book.asks px id := by
      simp [dequeue, setLevels, levelsOf]
    rw [hbids] at hb
    rw [hasks] at ha
    rcases best_removeAt_asc (by simpa [bestFirst] using hask) ha with ⟨a, ha0, hle⟩
    have hba : bid < a := hu bid a hb ha0
    omega

theorem uncrossed_levels {a b : Book} (hu : uncrossed a) (hb : b.bids = a.bids)
    (ha : b.asks = a.asks) : uncrossed b := by
  intro bid ask hbid hask
  exact hu bid ask (hb ▸ hbid) (ha ▸ hask)

theorem uncrossed_cancel (hinv : BookInv book) (hu : uncrossed book) (id : Nat) :
    uncrossed (cancelOrder book id) := by
  cases hfind : findOrder book.orders id with
  | none =>
    simpa [cancelOrder, hfind] using hu
  | some o =>
    cases hact : o.status.isActive
    · simpa [cancelOrder, hfind, hact] using hu
    · let o' : Order := { o with remaining := 0, status := .cancelled }
      have huW : uncrossed (writeOrder book o') :=
        uncrossed_levels hu (by simp [writeOrder]) (by simp [writeOrder])
      have hbid : bestFirst .buy (writeOrder book o').bids := by
        simpa [writeOrder, bestFirst] using hinv.bidOrder
      have hask : bestFirst .sell (writeOrder book o').asks := by
        simpa [writeOrder, bestFirst] using hinv.askOrder
      have huD := uncrossed_dequeue huW hbid hask o.side o.price id
      simpa [cancelOrder, hfind, hact, o'] using huD

theorem uncrossed_fill (hinv : BookInv book) (hu : uncrossed book) (incoming resting : Order)
    (policy : StpPolicy) {px : Int} {rid n : Nat} {rest : List Nat} {tail : List Level}
    (hsucc : incoming.remaining = Nat.succ n)
    (hbest : bestPrice (levelsOf book incoming.side.opposite) = some px)
    (hcross : pricesCross incoming.side incoming.price px = true)
    (hfront : frontAt (levelsOf book incoming.side.opposite) px = some rid)
    (hfind : findOrder book.orders rid = some resting)
    (_hshape : levelsOf book incoming.side.opposite =
      { price := px, orders := rid :: rest } :: tail)
    (hact : resting.status.isActive = true) (hrest : 0 < resting.remaining)
    (hconf : stpConflict incoming resting policy = false) :
    uncrossed (step book incoming policy false).book := by
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
  have hu1 : uncrossed book1 :=
    uncrossed_levels hu (by simp [book1, writeOrder, emitted, emitTrade])
      (by simp [book1, writeOrder, emitted, emitTrade])
  have hbid1 : bestFirst .buy book1.bids := by
    simpa [book1, writeOrder, emitted, emitTrade, bestFirst] using hinv.bidOrder
  have hask1 : bestFirst .sell book1.asks := by
    simpa [book1, writeOrder, emitted, emitTrade, bestFirst] using hinv.askOrder
  have hu2 : uncrossed book2 := by
    cases hdead : (resting'.remaining == 0)
    · simpa [book2, hdead] using hu1
    · simpa [book2, hdead] using uncrossed_dequeue hu1 hbid1 hask1 opp px rid
  simpa [hsEq] using hu2

theorem uncrossed_step (hinv : BookInv book) (hu : uncrossed book) (incoming : Order)
    (policy : StpPolicy) (stp : Bool) : uncrossed (step book incoming policy stp).book := by
  cases stp
  case true =>
    have hsEq : step book incoming policy true =
        { book, incoming, stp := true, trade := none, go := false } := by
      simp [step]
    simpa [hsEq] using hu
  case false =>
    cases hsucc : incoming.remaining
    case zero =>
      have hsEq : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc]
      simpa [hsEq] using hu
    case succ n =>
      let opp := incoming.side.opposite
      cases hbest : bestPrice (levelsOf book opp)
      case none =>
        have hsEq : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hsucc, hbest, opp]
        simpa [hsEq] using hu
      case some px =>
        cases hcross : pricesCross incoming.side incoming.price px
        case false =>
          have hsEq : step book incoming policy false =
              { book, incoming, stp := false, trade := none, go := false } := by
            simp [step, hsucc, hbest, hcross, opp]
          simpa [hsEq] using hu
        case true =>
          rcases front_order hinv opp hbest with
            ⟨rid, resting, _rest, _tail, hshape, hfront, hfind, _hside, _hpx, hact, hrest⟩
          have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
          cases policy
          case off =>
            exact uncrossed_fill hinv hu incoming resting .off hsucc hbest hcross hfront hfind
              hshape hact hrest (stpConflict_off incoming resting)
          case cancelNewest =>
            cases hconf : stpConflict incoming resting .cancelNewest
            case false =>
              exact uncrossed_fill hinv hu incoming resting .cancelNewest hsucc hbest hcross hfront
                hfind hshape hact hrest hconf
            case true =>
              have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
              have hsEq : step book incoming .cancelNewest false =
                  { book, incoming := stopIncoming incoming, stp := true, trade := none,
                    go := false } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq, opp]
              simpa [hsEq] using hu
          case cancelOldest =>
            cases hconf : stpConflict incoming resting .cancelOldest
            case false =>
              exact uncrossed_fill hinv hu incoming resting .cancelOldest hsucc hbest hcross hfront
                hfind hshape hact hrest hconf
            case true =>
              have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
              have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
              have hsEq : step book incoming .cancelOldest false =
                  { book := cancelOrder book rid, incoming, stp := false, trade := none,
                    go := true } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hsame,
                  opp]
              simpa [hsEq] using uncrossed_cancel hinv hu rid
          case decrementAndCancel =>
            cases hconf : stpConflict incoming resting .decrementAndCancel
            case false =>
              exact uncrossed_fill hinv hu incoming resting .decrementAndCancel hsucc hbest hcross
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
                simpa [hsEq] using hu
              · have hnotN : ¬ n + 1 < resting.remaining := by simpa [hsucc] using hltR
                have hsEq : step book incoming .decrementAndCancel false =
                    { book := cancelOrder book rid, incoming, stp := false, trade := none,
                      go := true } := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hnotOld, hdec, hnotN, opp]
                simpa [hsEq] using uncrossed_cancel hinv hu rid

theorem uncrossed_matchLoop (fuel : Nat) (hinv : BookInv book) (hu : uncrossed book)
    (incoming : Order) (policy : StpPolicy) (rev : List Trade) (stp : Bool) :
    uncrossed (matchLoop fuel book incoming policy rev stp).1 := by
  induction fuel generalizing book incoming rev stp with
  | zero => simpa [matchLoop] using hu
  | succ n ih =>
    have hS := uncrossed_step hinv hu incoming policy stp
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

theorem step_stopped (hinv : BookInv book) (incoming : Order) (policy : StpPolicy) (stp : Bool)
    (hgo : (step book incoming policy stp).go = false) :
    matchedDone (step book incoming policy stp).book (step book incoming policy stp).incoming
      (step book incoming policy stp).stp := by
  cases stp
  case true =>
    have hsEq : step book incoming policy true =
        { book, incoming, stp := true, trade := none, go := false } := by
      simp [step]
    rw [hsEq]
    exact Or.inl rfl
  case false =>
    cases hsucc : incoming.remaining
    case zero =>
      have hsEq : step book incoming policy false =
          { book, incoming, stp := false, trade := none, go := false } := by
        simp [step, hsucc]
      rw [hsEq]
      exact Or.inr (Or.inl hsucc)
    case succ n =>
      let opp := incoming.side.opposite
      cases hbest : bestPrice (levelsOf book opp)
      case none =>
        have hsEq : step book incoming policy false =
            { book, incoming, stp := false, trade := none, go := false } := by
          simp [step, hsucc, hbest, opp]
        rw [hsEq]
        refine Or.inr (Or.inr ?_)
        intro px hpx
        simp [hbest, opp] at hpx
      case some px =>
        cases hcross : pricesCross incoming.side incoming.price px
        case false =>
          have hsEq : step book incoming policy false =
              { book, incoming, stp := false, trade := none, go := false } := by
            simp [step, hsucc, hbest, hcross, opp]
          rw [hsEq]
          refine Or.inr (Or.inr ?_)
          intro px' hpx
          have hsame : px = px' := by
            simpa [hbest, opp] using hpx
          simpa [hsame] using hcross
        case true =>
          rcases front_order hinv opp hbest with
            ⟨rid, resting, _rest, _tail, _hshape, hfront, hfind, _hside, _hpx, hact, hrest⟩
          have hne : resting.remaining ≠ 0 := Nat.ne_of_gt hrest
          cases policy
          case off =>
            have hsEq : step book incoming .off false =
                { book := (step book incoming .off false).book, incoming := fill incoming
                    (min incoming.remaining resting.remaining), stp := false,
                  trade := some (emitTrade book px (min incoming.remaining resting.remaining)
                    incoming.id rid incoming.side).2,
                  go := true } := by
              simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, opp,
                stpConflict_off incoming resting]
            rw [hsEq] at hgo
            cases hgo
          case cancelNewest =>
            cases hconf : stpConflict incoming resting .cancelNewest
            case false =>
              have hconf' : stpConflict incoming resting .cancelNewest = false := hconf
              have hsEq : (step book incoming .cancelNewest false).go = true := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf', opp]
              rw [hsEq] at hgo
              cases hgo
            case true =>
              have hbeq : (StpPolicy.cancelNewest == .cancelNewest) = true := by decide
              have hsEq : step book incoming .cancelNewest false =
                  { book, incoming := stopIncoming incoming, stp := true, trade := none,
                    go := false } := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hbeq, opp]
              rw [hsEq]
              exact Or.inl rfl
          case cancelOldest =>
            cases hconf : stpConflict incoming resting .cancelOldest
            case false =>
              have hsEq : (step book incoming .cancelOldest false).go = true := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, opp]
              rw [hsEq] at hgo
              cases hgo
            case true =>
              have hnotNew : (StpPolicy.cancelOldest == .cancelNewest) = false := by decide
              have hsame : (StpPolicy.cancelOldest == .cancelOldest) = true := by decide
              have hsEq : (step book incoming .cancelOldest false).go = true := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew, hsame,
                  opp]
              rw [hsEq] at hgo
              cases hgo
          case decrementAndCancel =>
            cases hconf : stpConflict incoming resting .decrementAndCancel
            case false =>
              have hsEq : (step book incoming .decrementAndCancel false).go = true := by
                simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, opp]
              rw [hsEq] at hgo
              cases hgo
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
                rw [hsEq]
                exact Or.inl rfl
              · have hnotN : ¬ n + 1 < resting.remaining := by simpa [hsucc] using hltR
                have hsEq : (step book incoming .decrementAndCancel false).go = true := by
                  simp [step, hsucc, hbest, hcross, hfront, hfind, hact, hne, hconf, hnotNew,
                    hnotOld, hdec, hnotN, opp]
                rw [hsEq] at hgo
                cases hgo

theorem matchFuel_zero_remaining {book : Book} {incoming : Order}
    (h : matchFuel book incoming = 0) : incoming.remaining = 0 := by
  simp only [matchFuel] at h
  omega

theorem matchLoop_done_le (fuel : Nat) (hinv : BookInv book) (incoming : Order)
    (hfuel : matchFuel book incoming ≤ fuel) (policy : StpPolicy)
    (rev : List Trade) (stp : Bool) :
    matchedDone (matchLoop fuel book incoming policy rev stp).1
      (matchLoop fuel book incoming policy rev stp).2.1
      (matchLoop fuel book incoming policy rev stp).2.2.2 := by
  induction fuel generalizing book incoming rev stp with
  | zero =>
    have h0 : matchFuel book incoming = 0 := Nat.eq_zero_of_le_zero hfuel
    rw [matchLoop]
    exact Or.inr (Or.inl (matchFuel_zero_remaining h0))
  | succ n ih =>
    have hs := step_progress hinv incoming policy stp
    cases hgo : (step book incoming policy stp).go
    · rw [matchLoop]
      simp only [hgo]
      exact step_stopped hinv incoming policy stp hgo
    · have hlt := hs.fuel hgo
      have hle : matchFuel (step book incoming policy stp).book
          (step book incoming policy stp).incoming ≤ n := by omega
      have hrec := ih hs.inv (step book incoming policy stp).incoming hle
        (match (step book incoming policy stp).trade with
          | none => rev
          | some t => t :: rev)
        (step book incoming policy stp).stp
      rw [matchLoop]
      simp only [hgo]
      exact hrec

/-- Extra fuel does not change the stopped sweep. -/
theorem matchLoop_done (hinv : BookInv book) (incoming : Order) (policy : StpPolicy)
    (rev : List Trade) (stp : Bool) (k : Nat) :
    matchedDone (matchLoop (matchFuel book incoming + k) book incoming policy rev stp).1
      (matchLoop (matchFuel book incoming + k) book incoming policy rev stp).2.1
      (matchLoop (matchFuel book incoming + k) book incoming policy rev stp).2.2.2 := by
  rw [matchLoop_fuel hinv incoming policy rev stp k]
  exact matchLoop_done_le (matchFuel book incoming) hinv incoming (Nat.le_refl _) policy rev stp

theorem done_cross {book : Book} {o : Order} (hd : matchedDone book o false)
    (hrem : o.remaining ≠ 0) :
    ∀ px, bestPrice (levelsOf book o.side.opposite) = some px →
      pricesCross o.side o.price px = false := by
  rcases hd with hstp | hzero | hcross
  · cases hstp
  · exact absurd hzero hrem
  · exact hcross

theorem uncrossed_settle (hu : uncrossed book) (o : Order) (stp : Bool)
    (hd : matchedDone book o stp) : uncrossed (settle book o stp) := by
  cases hstp : stp
  · cases hzero : (o.remaining == 0)
    · cases htif : (o.timeInForce == .gtc)
      · exact uncrossed_levels hu (by simp [settle, hzero, htif, writeOrder])
          (by simp [settle, hzero, htif, writeOrder])
      · let status := if o.filled > 0 then OrderStatus.partiallyFilled else OrderStatus.new
        let o' : Order := { o with status := status }
        have hrem : o.remaining ≠ 0 := by simpa using hzero
        have hsafe := done_cross (by simpa [hstp] using hd) hrem
        have huW : uncrossed (writeOrder book o') :=
          uncrossed_levels hu (by simp [writeOrder]) (by simp [writeOrder])
        have huE := uncrossed_enqueue huW o hsafe
        simpa [settle, hstp, hzero, htif, status, o'] using huE
    · exact uncrossed_levels hu (by simp [settle, hzero, writeOrder])
        (by simp [settle, hzero, writeOrder])
  · exact uncrossed_levels hu (by simp [settle, writeOrder])
      (by simp [settle, writeOrder])

theorem uncrossed_alloc (hu : uncrossed book) (side : Side) (price : Int) (quantity : Nat)
    (tif : TimeInForce) (owner : Option Nat) :
    uncrossed (alloc book side price quantity tif owner).1 :=
  uncrossed_levels hu (by simp [alloc]) (by simp [alloc])

theorem uncrossed_submit (hinv : BookInv book) (hu : uncrossed book) (side : Side) (price : Int)
    (quantity : Nat) (tif : TimeInForce) (owner : Option Nat) (policy : StpPolicy) :
    uncrossed (submit book side price quantity tif owner policy).1 := by
  unfold submit
  split
  · simpa using uncrossed_alloc hu side price quantity tif owner
  · let out := alloc book side price quantity tif owner
    have halloc := bookInv_alloc hinv side price quantity tif owner
    have huA : uncrossed out.1 := uncrossed_alloc hu side price quantity tif owner
    let looped := matchLoop (matchFuel out.1 out.2) out.1 out.2 policy [] false
    have huL : uncrossed looped.1 :=
      uncrossed_matchLoop (matchFuel out.1 out.2) halloc.1 huA out.2 policy [] false
    have hd := matchLoop_done halloc.1 out.2 policy [] false 0
    have hdL : matchedDone looped.1 looped.2.1 looped.2.2.2 := by
      simpa [Nat.add_zero, out, looped] using hd
    simpa [out, looped] using uncrossed_settle huL looped.2.1 looped.2.2.2 hdL

theorem uncrossed_apply (hinv : BookInv book) (hu : uncrossed book) (event : Event) :
    uncrossed (apply book event).1 := by
  cases event with
  | submitLimit side price quantity tif owner policy =>
    simpa [apply] using uncrossed_submit hinv hu side price quantity tif owner policy
  | submitMarket side quantity owner policy =>
    cases side with
    | buy => simpa [apply] using uncrossed_submit hinv hu .buy i64Max quantity .ioc owner policy
    | sell => simpa [apply] using uncrossed_submit hinv hu .sell i64Min quantity .ioc owner policy
  | cancel id =>
    simpa [apply] using uncrossed_cancel hinv hu id

theorem uncrossed_foldl (hinv : BookInv book) (hu : uncrossed book) (events : List Event) :
    uncrossed (events.foldl (fun b ev => (apply b ev).1) book) := by
  induction events generalizing book with
  | nil => simpa using hu
  | cons event rest ih =>
    exact ih (bookInv_apply hinv event) (uncrossed_apply hinv hu event)

theorem uncrossed_empty : uncrossed Book.empty := by
  intro _ _ hbid
  simp [Book.empty, bestPrice] at hbid

theorem uncrossed_reach (events : List Event) : uncrossed (reach events) := by
  simpa [reach] using uncrossed_foldl bookInv_empty uncrossed_empty events

theorem law2_holds : law2 := by
  intro book hreach bid ask hbid hask
  rcases hreach with ⟨events, rfl⟩
  exact uncrossed_reach events bid ask hbid hask

end Nanobook
