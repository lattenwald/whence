-module(z).
-export([f/1]).

f(K) ->
    Z = case K of
            1 -> one;
            _ -> two
        end,
    Z.
