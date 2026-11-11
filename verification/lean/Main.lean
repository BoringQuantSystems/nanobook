import Lean.Data.Json
import Nanobook.Spec

/-
Read a corpus `input.jsonl` and write the trade lines.
-/

open Nanobook

def u64Max : Nat := 18446744073709551615

def sideName : Side → String
  | .buy => "BUY"
  | .sell => "SELL"

def parseSide (s : String) : Except String Side :=
  match s with
  | "BUY" => .ok .buy
  | "SELL" => .ok .sell
  | _ => .error "invalid side"

def parseTif (s : String) : Except String TimeInForce :=
  match s with
  | "GTC" => .ok .gtc
  | "IOC" => .ok .ioc
  | "FOK" => .ok .fok
  | _ => .error "invalid time_in_force"

def parsePolicy (s : String) : Except String StpPolicy :=
  match s with
  | "Off" => .ok .off
  | "CancelNewest" => .ok .cancelNewest
  | "CancelOldest" => .ok .cancelOldest
  | "DecrementAndCancel" => .ok .decrementAndCancel
  | _ => .error "invalid stp_policy"

def checkPrice (n : Int) : Except String Int :=
  if n < i64Min || n > i64Max then .error "invalid price" else .ok n

def parsePrice (v : Lean.Json) : Except String Int :=
  match v with
  | .str s =>
    match s.toInt? with
    | some n => checkPrice n
    | none => .error "invalid price"
  | .num n =>
    if n.exponent != 0 then .error "invalid price" else checkPrice n.mantissa
  | _ => .error "invalid price"

def parseQty (v : Lean.Json) : Except String Nat :=
  match v with
  | .num n =>
    if n.exponent != 0 || n.mantissa < 0 then
      .error "invalid quantity"
    else
      let q := n.mantissa.toNat
      if q > u64Max then .error "invalid quantity" else .ok q
  | _ => .error "invalid quantity"

def parseOwner (v : Lean.Json) : Except String (Option Nat) :=
  match v with
  | .null => .ok none
  | .num n =>
    -- Same contract as corpus-replay: an owner must fit in u32.
    if n.exponent != 0 || n.mantissa < 0 || n.mantissa.toNat > 4294967295 then
      .error "invalid owner"
    else
      .ok (some n.mantissa.toNat)
  | _ => .error "invalid owner"

def field (j : Lean.Json) (key : String) : Except String Lean.Json :=
  j.getObjVal? key

def optField (j : Lean.Json) (key : String) : Lean.Json :=
  j.getObjValD key

/-- An absent `stp_policy` means `Off`; an explicit `null` is invalid, as in corpus-replay. -/
def parsePolicyField (j : Lean.Json) : Except String StpPolicy :=
  match j.getObjVal? "stp_policy" with
  | .error _ => .ok .off
  | .ok v => do parsePolicy (← v.getStr?)

def parseEvent (line : String) : Except String Event := do
  let j ← Lean.Json.parse line
  let ty ← (← field j "type").getStr?
  match ty with
  | "SubmitLimit" =>
    let side ← parseSide (← (← field j "side").getStr?)
    let price ← parsePrice (← field j "price")
    let quantity ← parseQty (← field j "quantity")
    let tif ← parseTif (← (← field j "time_in_force").getStr?)
    let owner ← parseOwner (optField j "owner")
    let policy ← parsePolicyField j
    pure (.submitLimit side price quantity tif owner policy)
  | "SubmitMarket" =>
    let side ← parseSide (← (← field j "side").getStr?)
    let quantity ← parseQty (← field j "quantity")
    let owner ← parseOwner (optField j "owner")
    let policy ← parsePolicyField j
    pure (.submitMarket side quantity owner policy)
  | "Cancel" =>
    let orderId ← parseQty (← field j "order_id")
    pure (.cancel orderId)
  | _ => .error s!"unknown event type: {ty}"

def parseLines (lines : List String) : Except String (List Event) :=
  match lines with
  | [] => .ok []
  | h :: rest =>
    let trimmed := h.trimAscii.copy
    if trimmed.isEmpty then
      parseLines rest
    else
      match parseEvent trimmed with
      | .ok e =>
        match parseLines rest with
        | .ok tail => .ok (e :: tail)
        | .error err => .error err
      | .error err => .error err
termination_by lines

def parseFile (text : String) : Except String (List Event) :=
  parseLines ((text.replace "\r" "").splitOn "\n")

def tradeLine (t : Trade) : String :=
  s!"\{\"id\":{t.id},\"price\":{t.price},\"quantity\":{t.quantity},\"aggressor_order_id\":{t.aggressorId},\"passive_order_id\":{t.passiveId},\"aggressor_side\":\"{sideName t.aggressorSide}\",\"timestamp\":{t.timestamp}}"

def render (trades : List Trade) : String :=
  match trades with
  | [] => ""
  | _ => String.intercalate "\n" (trades.map tradeLine) ++ "\n"

def main (args : List String) : IO UInt32 := do
  match args with
  | [input, output] =>
    let text ← IO.FS.readFile input
    match parseFile text with
    | .ok events =>
      IO.FS.writeFile output (render (replay events))
      return 0
    | .error err =>
      IO.eprintln err
      return 1
  | _ =>
    IO.eprintln "Usage: nanobook-replay <input.jsonl> <output.jsonl>"
    return 1
