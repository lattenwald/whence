-module(n).
-export([q/0]).

q() ->
    R = pick(3),
    R.

pick(N) when N > 5 -> N;
pick(N) -> N.
