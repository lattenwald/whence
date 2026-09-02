-module(sample).
-export([handle/2, pick/1]).

-record(req, {body, peer}).

handle(Req0, Opts) ->
    Body = read_body(Req0),
    Peer = Req0#req.peer,
    R = #req{body = Body, peer = Peer},
    Limit = maps:get(limit, Opts, 10),
    _F = fun(X) -> X end,
    _G = begin io:format("x"), 42 end,
    case pick(Limit) of
        {ok, V} -> {V, R};
        error -> {0, R}
    end.

pick(N) when N > 5 -> {ok, N * 2};
pick(_) -> error.

read_body(#req{body = B}) -> B.

cons() ->
    [H | T] = [1, 2, 3],
    [P, Q] = [1, 2, 3],
    E = {[], {}},
    {H, T, E, P, Q}.

compound(V) ->
    {A = B, C} = V,
    {A, B, C}.

wrap(X) ->
    case X of
        _ -> fun() -> 1 end
    end.

nearby(X, M) ->
    case pick(X) of
        _ -> tag(other(), maps:get(k, M))
    end.

recv(X) ->
    receive
        {msg, W} -> case X of _ -> W end
    end.

sock(State) -> State#state.conn#conn.sock.
