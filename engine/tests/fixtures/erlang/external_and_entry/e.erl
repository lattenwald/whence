-module(e).
-export([handle/1]).

handle(Req) ->
    B = os:getenv(Req),
    B.
