# The Qubit

@[section]({
    "id": "qubit__overview",
    "title": "Overview"
})

This kata introduces you to one of the core concepts in quantum computing - the qubit, and its representation in mathematical notation and in Q# code.

**This kata covers the following topics:**

- The concept of a qubit
- Superposition
- Vector representation of qubit states
- Dirac notation for single-qubit states
- Relative and global phase
- `Qubit` data type in Q#
- Visualizing the quantum state using `DumpMachine`

**What you should know to start working on this kata:**

- Basic knowledge of complex arithmetic
- Basic knowledge of linear algebra

@[section]({
    "id": "qubit__concept",
    "title": "The Concept of Qubit"
})

The basic building block of a classical computer is the bit - a single memory cell that is either in state $0$ or in state $1$. Similarly, the basic building block of a quantum computer is the quantum bit, or **qubit**. Like the classical bit, a qubit can be in state $0$ or in state $1$. Unlike the classical bit, however, the qubit isn't limited to just those two states - it may also be in a combination, or **superposition** of those states.

> A common misconception about quantum computing is that a qubit is always in state $1$ or state $0$, and we just don't know which one until we "measure" it. That's not the case. A qubit in a superposition is in a linear combination of the states 0 and 1. When a qubit is measured, it's forced to collapse into one state or the other - in other words, measuring a qubit is an irreversible process that changes its initial state.

## Matrix Representation

The state of a qubit is represented by a complex vector of size 2:

$$\begin{bmatrix} \alpha \\ \beta \end{bmatrix}$$

Here $\alpha$ and $\beta$ are complex numbers. $\alpha$ represents how "close" the qubit is to state $0$, and $\beta$ represents how "close" the qubit is to state $1$. This vector is normalized: $|\alpha|^2 + |\beta|^2 = 1$.
$\alpha$ and $\beta$ are known as the probability amplitudes of states $0$ and $1$, respectively.

## Basis States

A qubit in state $0$ would be represented by the following vector:

$$\begin{bmatrix} 1 \\ 0 \end{bmatrix}$$

Likewise, a qubit in state $1$ would be represented by this vector:

$$\begin{bmatrix} 0 \\ 1 \end{bmatrix}$$

Note that you can use scalar multiplication and vector addition to express any qubit state $\begin{bmatrix} \alpha \\ \beta \end{bmatrix}$ as a sum of these two vectors with certain weights $\alpha$ and $\beta$, known as linear combination.

$$
\begin{bmatrix} \alpha \\ \beta \end{bmatrix} =
\begin{bmatrix} \alpha \\ 0 \end{bmatrix} + \begin{bmatrix} 0 \\ \beta \end{bmatrix} =
\alpha \cdot \begin{bmatrix} 1 \\ 0 \end{bmatrix} + \beta \cdot \begin{bmatrix} 0 \\ 1 \end{bmatrix}
$$

Because of this, qubit states $0$ and $1$ are known as **basis states**. These two vectors have two properties.

1. They are normalized.

    $$
    \langle \begin{bmatrix} 1 \\ 0 \end{bmatrix} , \begin{bmatrix} 1 \\ 0 \end{bmatrix} \rangle =
    \langle \begin{bmatrix} 0 \\ 1 \end{bmatrix} , \begin{bmatrix} 0 \\ 1 \end{bmatrix} \rangle = 1
    $$

2. They are orthogonal to each other.

    $$
    \langle \begin{bmatrix} 1 \\ 0 \end{bmatrix} , \begin{bmatrix} 0 \\ 1 \end{bmatrix} \rangle =
    \langle \begin{bmatrix} 0 \\ 1 \end{bmatrix} , \begin{bmatrix} 1 \\ 0 \end{bmatrix} \rangle = 0
    $$

> As a reminder, $\langle V , W \rangle$ is the inner product of $V$ and $W$.

This means that these vectors form an **orthonormal basis**. The basis of $\begin{bmatrix} 1 \\ 0 \end{bmatrix}$ and $\begin{bmatrix} 0 \\ 1 \end{bmatrix}$ is called the **computational basis**, also known as the **canonical basis**.

> There exist other orthonormal bases, for example, the **Hadamard basis**, formed by the vectors
>
> $$\begin{bmatrix} \frac{1}{\sqrt{2}} \\ \frac{1}{\sqrt{2}} \end{bmatrix} \text{ and } \begin{bmatrix} \frac{1}{\sqrt{2}} \\ -\frac{1}{\sqrt{2}} \end{bmatrix}$$
>
> You can check that these vectors are normalized, and orthogonal to each other. Any qubit state can be expressed as a linear combination of these vectors:
>
> $$
> \begin{bmatrix} \alpha \\ \beta \end{bmatrix} =
> \frac{\alpha + \beta}{\sqrt{2}} \begin{bmatrix} \frac{1}{\sqrt{2}} \\ \frac{1}{\sqrt{2}} \end{bmatrix} +
> \frac{\alpha - \beta}{\sqrt{2}} \begin{bmatrix} \frac{1}{\sqrt{2}} \\ -\frac{1}{\sqrt{2}} \end{bmatrix}
> $$
>
> The Hadamard basis is widely used in quantum computing, for example, in the <a href="https://en.wikipedia.org/wiki/BB84" target="_blank">BB84 quantum key distribution protocol</a>.

@[section]({
    "id": "qubit__dirac_notation",
    "title": "Dirac Notation"
})

Dirac notation is a shorthand notation that eases writing quantum states and computing linear algebra. In Dirac notation, a vector is denoted by a symbol called a **ket**. For example, a qubit in state $0$ is represented by the ket $\ket{0}$, and a qubit in state $1$ is represented by the ket $\ket{1}$:

<table>
    <tr>
        <td>$$\ket{0} = \begin{bmatrix} 1 \\ 0 \end{bmatrix}$$</td>
        <td>$$\ket{1} = \begin{bmatrix} 0 \\ 1 \end{bmatrix}$$</td>
    </tr>
</table>

The kets $\ket{0}$ and $\ket{1}$ represent basis states, so they can be used to represent any other state:

$$\begin{bmatrix} \alpha \\ \beta \end{bmatrix} = \alpha\ket{0} + \beta\ket{1}$$

Dirac notation isn't restricted to vectors $0$ and $1$; it can be used to represent any vector, similar to how variable names are used in algebra. For example, you can call the above state "$\psi$" and write it as:

$$\ket{\psi} = \alpha\ket{0} + \beta\ket{1}$$

Several ket symbols have a generally accepted use, so you will see them often. For example, the following kets are commonly used:

<table>
    <tr>
        <td>$$\ket{+} = \frac{1}{\sqrt{2}}\big(\ket{0} + \ket{1}\big)$$</td>
        <td>$$\ket{-} = \frac{1}{\sqrt{2}}\big(\ket{0} - \ket{1}\big)$$</td>
    </tr>
    <tr>
        <td>$$\ket{i} = \frac{1}{\sqrt{2}}\big(\ket{0} + i\ket{1}\big)$$</td>
        <td>$$\ket{-i} = \frac{1}{\sqrt{2}}\big(\ket{0} - i\ket{1}\big)$$</td>
    </tr>
</table>

You will learn more about Dirac notation in the next katas, as you get introduced to quantum gates and multi-qubit systems.

@[section]({
    "id": "qubit__relative_and_global_phase",
    "title": "Relative and Global Phase"
})

Complex numbers have a parameter called the phase. If a complex number $z = x + iy$ is written in polar form $z = re^{i\theta}$, its phase is $\theta$, where $\theta = atan2(y, x)$.

> `atan2` is a useful function available in most programming languages. It takes two arguments and returns an angle $\theta$
> between $-\pi$ and $\pi$ that has $\cos \theta = x$ and $\sin \theta = y$. Unlike using $\tan^{-1}(\frac{y}{x})$, `atan2` computes
> the correct quadrant for the angle, since it preserves information about the signs of both sine and cosine of the angle.

The probability amplitudes $\alpha$ and $\beta$ are complex numbers, therefore $\alpha$ and $\beta$ have a phase. For example, consider a qubit in state $\frac{1 + i}{2}\ket{0} + \frac{1 - i}{2}\ket{1}$. If you do the math, you see that the phase of $\ket{0}$ is $atan2(\frac12, \frac12) = \frac{\pi}{4}$, and the phase of $\ket{1}$ is $atan2(\frac12, -\frac12) = -\frac{\pi}{4}$. The difference between these two phases is known as **relative phase**.

Multiplying the state of the entire system by $e^{i\theta}$ doesn't affect the relative phase: $\alpha\ket{0} + \beta\ket{1}$ has the same relative phase as $e^{i\theta}\big(\alpha\ket{0} + \beta\ket{1}\big)$. In the second expression, $\theta$ is known as the system's **global phase**.

The state of a qubit (or, more generally, the state of a quantum system) is defined by its relative phase - global phase arises as a consequence of using linear algebra to represent qubits, and has no physical meaning. That is, applying a phase to the entire state of a system (multiplying the entire vector by $e^{i\theta}$ for any real $\theta$) doesn't actually affect the state of the system. Because of this, global phase is sometimes known as **unobservable phase** or **hidden phase**.

@[section]({
    "id": "qubit__qsharp_data_type",
    "title": "Q# Qubit Data Type"
})

In Q#, qubits are represented by the `Qubit` data type. On a physical quantum computer, it's impossible to directly access the state of a qubit, whether to read its exact state, or to set it to a desired state, and this data type reflects that. Instead, you can change the state of a qubit using quantum gates, and extract information about the state of the system using measurements.

That being said, when you run Q# code on a quantum simulator instead of a physical quantum computer, you can use diagnostic functions that allow you to peek at the state of the quantum system. This is very useful both for learning and for debugging small Q# programs.

Qubits aren't an ordinary data type, so the variables of this type have to be declared and initialized ("allocated") a little differently. The `use` statement allocates a qubit (or multiple) that can be used until the end of the scope in which the statement was used: `use q = Qubit();` allocates a qubit and binds it to the variable `q`.

Freshly allocated qubits start out in state $\ket{0}$, and have to be returned to that state by the time they are released. If you attempt to release a qubit in any state other than $\ket{0}$, it will result in a runtime error. You will see why it is important later, when you look at multi-qubit systems.

## Visualizing Quantum State

Before we continue, let's learn some techniques to visualize the quantum state of our qubits.

### Display the Quantum State of a Single-Qubit Program

Let's start with a simple scenario: a program that acts on a single qubit.
The state of the quantum system used by this program can be represented as a complex vector of length 2, or, using Dirac notation,

$$\begin{bmatrix} \alpha \\ \beta \end{bmatrix} = \alpha\ket{0} + \beta\ket{1}$$

If this program runs on a physical quantum system, there's no way to get the information about the values of $\alpha$ and $\beta$ at a certain point of the program execution from a single observation.
You would need to run the program repeatedly up to this point, perform a measurement on the system, and aggregate the results of multiple measurements to estimate $\alpha$ and $\beta$.

However, at the early stages of quantum program development the program typically runs on a simulator - a classical program which simulates the behavior of a small quantum system while having complete information about its internal state.
You can take advantage of this to do some non-physical things, such as peeking at the internals of the quantum system to observe its exact state without disturbing it!

The `DumpMachine` function from the `Microsoft.Quantum.Diagnostics` namespace allows you to do exactly that. The output of `DumpMachine` is accurate up to a global phase, and remember that global phase does not have any physical meaning. When using `DumpMachine`, you may see that all probability amplitudes are multiplied by some complex number compared to the state you're expecting.

### Demo: DumpMachine For Single-Qubit Systems

The following demo shows how to allocate a qubit and examine its state in Q#. You'll use `DumpMachine` to output the state of the system at any point in the program without affecting the state.

> Note that the Q# code doesn't have access to the output of `DumpMachine`, so you can't write any non-physical code in Q#!

@[example]({"id": "qubit__single_qubit_dump_machine_demo", "codePath": "./examples/SingleQubitDumpMachineDemo.qs"})

The exact behavior of the `RunExample` operation depends on the quantum simulator or processor you're using.

On the simulator used in these demos, this function prints the information on each basis state that has a non-zero amplitude, one basis state per row.
This includes information about the amplitude of the state, the probability of measuring that state, and the phase of the state.

Note that each row has the following format:

<table>
    <thead>
        <tr>
            <th>Basis State</th>
            <th>Amplitude</th>
            <th>Measurement Probability</th>
            <th>Phase</th>
        </tr>
    </thead>
</table>

For example, the state $\ket{0}$ would be represented as follows:

<table>
    <tbody>
        <tr>
            <td>|0⟩</td>
            <td>1.0000+0.0000𝑖</td>
            <td>100.0000%</td>
            <td>↑ 0.0000</td></tr>
    </tbody>
</table>

> It's important to note that although we talk about quantum systems in terms of their state, Q# does not have any representation of the quantum state in the language. Instead, state is an internal property of the quantum system, modified using gates. For more information, see <a href="https://learn.microsoft.com/azure/quantum/concepts-dirac-notation#q-gate-sequences-equivalent-to-quantum-states" target="_blank">Q# documentation on quantum states</a>.

@[exercise]({
    "id": "qubit__learn_single_qubit_state",
    "title": "Learn the State of a Single Qubit Using DumpMachine",
    "path": "./learn_single_qubit_state/"
})

@[section]({
    "id": "qubit__conclusion",
    "title": "Conclusion"
})

Congratulations! In this kata you learned the basics of qubits and qubit states. Here are a few key concepts to keep in mind:

- A qubit is a basic unit of quantum information, analogous to a bit in classical computing.
- Superposition is a quantum phenomenon where a qubit is in a combination of both 0 and 1 states. When measured, a qubit goes from being in superposition to one of the classical states.
- A qubit can be represented as $\ket{\psi} = \alpha\ket{0} + \beta\ket{1}$, where $\alpha$ and $\beta$ are complex numbers and state vectors $\ket{0}$ and $\ket{1}$ are $0$ and $1$ states respectively.
- In Q#, qubits are represented by the `Qubit` data type. When simulating a quantum program, you can use `DumpMachine` to inspect the state of a qubit without disturbing it.
