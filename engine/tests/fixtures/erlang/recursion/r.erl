-module(r).
-export([k/0, total/0]).

k() ->
    R = loop(7),
    R.

loop(S) -> loop(S).

total() ->
    S = sum([1, 2], 0),
    S.

sum([H | T], Acc) -> sum(T, Acc + H);
sum([], Acc) -> Acc.
