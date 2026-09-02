-module(p).
-export([shadow/1, through_call/0]).

-record(r, {a, b}).

shadow(X) ->
    R = #r{a = X},
    F = fun(R) -> A = R#r.a, A end,
    F(#r{a = inner}).

make(V) ->
    #r{a = V, b = V}.

through_call() ->
    R = make(1),
    R2 = R#r{a = 2},
    A = R#r.a,
    B = R2#r.a,
    C = R2#r.b,
    {A, B, C}.
