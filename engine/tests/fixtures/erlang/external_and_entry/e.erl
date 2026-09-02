-module(e).
-export([handle/1]).

handle(Req) ->
    B = cowboy_req:body(Req),
    B.
