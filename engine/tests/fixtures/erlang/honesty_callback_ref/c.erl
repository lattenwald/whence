-module(c).
-export([g/1]).

g(L) -> lists:map(fun cb/1, L).

cb(A) -> A.
