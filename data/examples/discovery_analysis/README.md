# Flip Flop Discovery Protocol Analysis

Discovery is a stochastic process.  For a station to be discovered it sends a message to the controller that (a) may collide on the wire with a message from another station and (b) may claim an address that another station claims.

In each round of discovery there is a an expected number of successes in phases (a) and (b) computed by `n * (1 - 1/m)^(n - 1)` where `n` is the number of remaining stations and `m` is the number of available time slots or remaining address slots.  Ref: https://math.stackexchange.com/q/35798  

There is also a probability of any collision (meaning another round will be required) ref: https://en.wikipedia.org/wiki/Birthday_problem


## Results

Using 400 time slots and 255 initially available addresses.

8 stations -> 1 rounds
32 stations -> 2 rounds
64 -> 3
128 -> 4
196 -> 6
255 -> 12


