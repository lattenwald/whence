-module(d).
-export([h/0, k/0]).

h() ->
    R = pick(3),
    R.

k() ->
    R = id(7),
    R.

pick(N) when N > 5 -> {ok, N};
pick(_) -> error.

id(N) -> N.
