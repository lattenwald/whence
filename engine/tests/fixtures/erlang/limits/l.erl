-module(l).
-export([p/1, loop/1]).

p(A) -> A.

loop(S) -> loop(S).
