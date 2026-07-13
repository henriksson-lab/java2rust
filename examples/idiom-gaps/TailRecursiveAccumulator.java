class TailRecursiveAccumulator {
    long factorial(int n) {
        return factorialLoop(n, 1L);
    }

    long factorialLoop(int n, long acc) {
        if (n <= 1) {
            return acc;
        }
        return factorialLoop(n - 1, acc * n);
    }
}
