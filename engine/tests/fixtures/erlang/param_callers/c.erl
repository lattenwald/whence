-module(c).
-export([run/0]).

run() ->
    Val = os:getenv("V"),
    b:g(1),
    b:g(Val).
