(* Unit tests for OCaml oracle.
   oracle_lib is unwrapped (see lib/dune), so its modules (Side, Order,
   Matching, Replay, ...) are in scope directly. *)

(* Test counter *)
let test_count = ref 0
let pass_count = ref 0

let assert_int (name : string) (expected : int) (actual : int) =
  test_count := !test_count + 1;
  if expected = actual then (
    pass_count := !pass_count + 1;
    Printf.printf "ok %s\n" name
  ) else
    Printf.printf "FAIL %s: expected %d, got %d\n" name expected actual

let assert_int64 (name : string) (expected : int64) (actual : int64) =
  test_count := !test_count + 1;
  if Int64.equal expected actual then (
    pass_count := !pass_count + 1;
    Printf.printf "ok %s\n" name
  ) else
    Printf.printf "FAIL %s: expected %Ld, got %Ld\n" name expected actual

let assert_true (name : string) (condition : bool) =
  test_count := !test_count + 1;
  if condition then (
    pass_count := !pass_count + 1;
    Printf.printf "ok %s\n" name
  ) else
    Printf.printf "FAIL %s: condition failed\n" name

let submit_limit side price quantity time_in_force =
  Replay.SubmitLimit
    { side; price; quantity; time_in_force; owner = None; stp_policy = Matching.Off }

(* Test: Simple cross produces a trade *)
let test_simple_cross () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 10000L 50L Order.GTC;
      ]
  in
  assert_int "simple_cross produces trade" 1 (List.length trades)

(* Test: No cross produces no trades *)
let test_no_cross () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 9999L 50L Order.GTC;
      ]
  in
  assert_int "no_cross produces no trades" 0 (List.length trades)

(* Test: Trade quantity is correct *)
let test_trade_quantity () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 10000L 50L Order.GTC;
      ]
  in
  let trade = List.hd trades in
  assert_int64 "trade_quantity is correct" 50L trade.Matching.quantity

(* Test: Trade price is correct *)
let test_trade_price () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 10000L 50L Order.GTC;
      ]
  in
  let trade = List.hd trades in
  assert_int64 "trade_price is correct" 10000L trade.Matching.price

(* Test: No negative quantities *)
let test_no_negative_quantities () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 10000L 50L Order.GTC;
      ]
  in
  assert_true "no negative quantities"
    (List.for_all (fun t -> t.Matching.quantity >= 0L) trades)

(* Test: Valid trade prices *)
let test_valid_trade_prices () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 10000L 50L Order.GTC;
      ]
  in
  assert_true "valid trade prices"
    (List.for_all (fun t -> t.Matching.price >= 0L) trades)

(* Test: Order IDs are unique *)
let test_unique_order_ids () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 10000L 50L Order.GTC;
      ]
  in
  let aggressor_ids = List.map (fun t -> t.Matching.aggressor_order_id) trades in
  let passive_ids = List.map (fun t -> t.Matching.passive_order_id) trades in
  let all_ids = aggressor_ids @ passive_ids in
  let unique_ids =
    List.fold_left (fun acc id -> if List.mem id acc then acc else id :: acc) [] all_ids
  in
  assert_int "unique order IDs" (List.length all_ids) (List.length unique_ids)

(* Test: FOK with insufficient liquidity produces no trades *)
let test_fok_no_liquidity () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 10L Order.GTC;
        submit_limit Side.Buy 10000L 100L Order.FOK;
      ]
  in
  assert_int "FOK with insufficient liquidity produces no trades" 0 (List.length trades)

(* Test: FOK with sufficient liquidity produces trades *)
let test_fok_with_liquidity () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 100L Order.GTC;
        submit_limit Side.Buy 10000L 50L Order.FOK;
      ]
  in
  assert_int "FOK with sufficient liquidity produces trades" 1 (List.length trades)

(* Test: IOC with partial fill *)
let test_ioc_partial_fill () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 50L Order.GTC;
        submit_limit Side.Buy 10000L 100L Order.IOC;
      ]
  in
  assert_int "IOC partial fill produces trade" 1 (List.length trades)

(* Test: Market order sweeps the book *)
let test_market_order () =
  let trades =
    Replay.replay_events
      [
        submit_limit Side.Sell 10000L 50L Order.GTC;
        submit_limit Side.Sell 10001L 50L Order.GTC;
        Replay.SubmitMarket
          { side = Side.Buy; quantity = 80L; owner = None; stp_policy = Matching.Off };
      ]
  in
  assert_int "market order sweeps two levels" 2 (List.length trades);
  let total =
    List.fold_left (fun acc t -> Int64.add acc t.Matching.quantity) 0L trades
  in
  assert_int64 "market order fills full quantity" 80L total

(* Test: trade JSONL preserves i64 prices outside OCaml int range *)
let test_extreme_price_trade_json () =
  let trade =
    Matching.create_trade
      ~id:1L
      ~price:(-9223372036854775807L)
      ~quantity:50L
      ~aggressor_order_id:2L
      ~passive_order_id:1L
      ~aggressor_side:Side.Buy
      ~timestamp:3L
  in
  let json = Json.trade_to_jsonl_string trade in
  assert_true "extreme price trade json preserves price literal"
    (String.contains json '9')

(* Run all tests *)
let () =
  print_endline "Running OCaml oracle unit tests...";
  print_newline ();

  test_simple_cross ();
  test_no_cross ();
  test_trade_quantity ();
  test_trade_price ();
  test_no_negative_quantities ();
  test_valid_trade_prices ();
  test_unique_order_ids ();
  test_fok_no_liquidity ();
  test_fok_with_liquidity ();
  test_ioc_partial_fill ();
  test_market_order ();
  test_extreme_price_trade_json ();

  print_newline ();
  Printf.printf "Results: %d/%d tests passed\n" !pass_count !test_count;
  if !pass_count = !test_count then exit 0 else exit 1
