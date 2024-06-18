Bob's side of this protocol can be represented as a sequence of two steps:
 1. Transform the Bell pair he shares with Alice back to $\ket{\Phi ^+}$. This can be done by applying the $X$ gate to Bob's qubit.
 2. Apply the standard teleportation protocol.
 
The final set of corrections we need is the $X$ gate, followed by the standard teleportation corrections. This adds up to the following corrections:
- For 00, only the $X$ correction is required.
- For 01, both $Z$ and $X$ corrections are required.
- For 10, no change is required.
- For 11, only the $Z$ correction is requried.

@[solution]({
    "id": "teleportation__reconstruct_message_psi_plus_solution",
    "codePath": "./Solution.qs"
})