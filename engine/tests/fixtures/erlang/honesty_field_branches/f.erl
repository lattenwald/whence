-module(f).
-export([pick/1]).

-record(r, {a}).

pick(K) ->
    case K of
        1 -> R = #r{a = one};
        _ -> R = #r{a = two}
    end,
    V = R#r.a,
    V.
