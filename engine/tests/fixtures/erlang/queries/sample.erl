-module(sample).
-export([handle/2, pick/1]).

-record(req, {body, peer}).

handle(Req0, Opts) ->
    Body = read_body(Req0),
    Peer = Req0#req.peer,
    R = #req{body = Body, peer = Peer},
    Limit = maps:get(limit, Opts, 10),
    _F = fun(X) -> X end,
    case pick(Limit) of
        {ok, V} -> {V, R};
        error -> {0, R}
    end.

pick(N) when N > 5 -> {ok, N * 2};
pick(_) -> error.

read_body(#req{body = B}) -> B.
