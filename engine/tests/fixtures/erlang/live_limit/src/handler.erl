-module(handler).
-export([handle/2, limit_for/1]).

-record(req, {body, peer}).

handle(Req0, Opts) ->
    Body = read_body(Req0),
    Limit = limit_for(Opts),
    Reply = build(Body, Limit),
    Reply.

limit_for(Opts) ->
    case maps:get(limit, Opts, undefined) of
        undefined -> default_limit();
        N -> N
    end.

build(Body, Limit) ->
    #req{body = Body, peer = Limit}.

read_body(#req{body = B}) -> B.

default_limit() -> 10.
