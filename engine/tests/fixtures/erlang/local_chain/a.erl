-module(a).
-export([f/1]).

f(X) ->
    Y = X,
    Z = Y,
    Z.
