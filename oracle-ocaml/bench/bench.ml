(* Performance benchmarks for the OCaml oracle.

   oracle_lib is unwrapped (see lib/dune), so its modules are in scope
   directly. Replay.replay_events owns its book and replays a full event
   list, so each benchmark builds the whole event list up front and times
   one replay over it. *)

(* Timing helper *)
let time f =
  let start = Unix.gettimeofday () in
  let result = f () in
  let finish = Unix.gettimeofday () in
  (result, finish -. start)

let submit_limit side price quantity =
  Replay.SubmitLimit
    {
      side;
      price;
      quantity;
      time_in_force = Order.GTC;
      owner = None;
      stp_policy = Matching.Off;
    }

(* Benchmark: simple cross throughput — n resting sells, n crossing buys *)
let bench_simple_cross n =
  Printf.printf "Benchmark: Simple cross (n=%d)\n" n;
  let sells = List.init n (fun _ -> submit_limit Side.Sell 10000L 100L) in
  let buys = List.init n (fun _ -> submit_limit Side.Buy 10000L 50L) in
  let events = sells @ buys in
  let trades, elapsed = time (fun () -> Replay.replay_events events) in
  Printf.printf "  Replay time: %.4f s\n" elapsed;
  Printf.printf "  Trades: %d\n" (List.length trades);
  Printf.printf "  Throughput: %.0f orders/sec\n"
    (float_of_int (2 * n) /. elapsed);
  print_newline ()

(* Benchmark: market order sweeping n price levels *)
let bench_market_sweep n =
  Printf.printf "Benchmark: Market order sweep (n=%d levels)\n" n;
  let resting =
    List.init n (fun i -> submit_limit Side.Sell (Int64.of_int (10000 + i)) 100L)
  in
  let sweep =
    Replay.SubmitMarket
      {
        side = Side.Buy;
        quantity = Int64.of_int (100 * n);
        owner = None;
        stp_policy = Matching.Off;
      }
  in
  let trades, elapsed = time (fun () -> Replay.replay_events (resting @ [ sweep ])) in
  Printf.printf "  Replay time: %.4f s\n" elapsed;
  Printf.printf "  Trades: %d\n" (List.length trades);
  Printf.printf "  Levels/sec: %.0f\n" (float_of_int n /. elapsed);
  print_newline ()

(* Benchmark: building a large book of non-crossing orders *)
let bench_large_book n =
  Printf.printf "Benchmark: Large order book (n=%d orders)\n" n;
  let events =
    List.init n (fun _ ->
        let side, price =
          if Random.int 2 = 0 then (Side.Buy, 9500 + Random.int 400)
          else (Side.Sell, 10100 + Random.int 400)
        in
        submit_limit side (Int64.of_int price) (Int64.of_int (10 + Random.int 90)))
  in
  let _, elapsed = time (fun () -> Replay.replay_events events) in
  Printf.printf "  Replay time: %.4f s\n" elapsed;
  Printf.printf "  Orders/sec: %.0f\n" (float_of_int n /. elapsed);
  print_newline ()

let () =
  print_endline "OCaml oracle benchmarks";
  print_newline ();
  Random.self_init ();
  bench_simple_cross 1_000;
  bench_market_sweep 1_000;
  bench_large_book 10_000
