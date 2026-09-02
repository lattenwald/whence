-module(g).
-export([handle/1]).

-record(req, {body, peer}).

handle(Req0) ->
    R = #req{peer = Req0#req.peer},
    P = R#req.peer,
    P.
