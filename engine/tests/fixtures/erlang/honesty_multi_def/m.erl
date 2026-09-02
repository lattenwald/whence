-module(m).
-export([h/1]).

h(K) ->
    case K of
        1 -> X = one;
        _ -> X = two
    end,
    Y = X,
    Y.
