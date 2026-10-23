package org.apache.commons.numbers.primes;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** Caller-only oracle; all primality and factorization algorithms are upstream. */
public final class PrimesOracle {
    interface Action { Object run(); }

    static void observe(String label, Action action) {
        try { System.out.println(label + "=" + action.run()); }
        catch (IllegalArgumentException error) {
            System.out.println(label + "=IllegalArgumentException:" + error.getMessage());
        }
    }

    static void publicOperations(int n) {
        observe("prime." + n, () -> Primes.isPrime(n));
        observe("next." + n, () -> Primes.nextPrime(n));
        observe("factors." + n, () -> Primes.primeFactors(n));
    }

    static void helpers() {
        int[] wheel = SmallPrimes.PRIME_NUMBERS_AND_COPRIME_EQUIVALENCE_CLASSES.getValue();
        System.out.println("table.primes=" + Arrays.toString(SmallPrimes.PRIMES));
        System.out.println("table.last=" + SmallPrimes.PRIMES_LAST);
        System.out.println("wheel.values=" + Arrays.toString(wheel));
        System.out.println("wheel.size=" + SmallPrimes.PRIME_NUMBERS_AND_COPRIME_EQUIVALENCE_CLASSES.getKey().size());
        for (int p : new int[] {2, 3, 5, 7, 11, 13})
            System.out.println("wheel.contains." + p + "=" + SmallPrimes.PRIME_NUMBERS_AND_COPRIME_EQUIVALENCE_CLASSES.getKey().contains(p));
        for (int n : new int[] {2, 64, 123456, 3673 * 3677, 46337 * 46337, Integer.MAX_VALUE}) {
            List<Integer> factors = new ArrayList<>();
            factors.add(71); // Helpers must append, not replace their supplied list.
            int remainder = SmallPrimes.smallTrialDivision(n, factors);
            System.out.println("small.remainder." + n + "=" + remainder);
            System.out.println("small.factors." + n + "=" + factors);
            System.out.println("trial." + n + "=" + SmallPrimes.trialDivision(n));
        }
        for (int n : new int[] {3673, 3673 * 3677, 46337 * 46337, Integer.MAX_VALUE}) {
            for (int bound : new int[] {3671, 3673, 46340}) {
                List<Integer> factors = new ArrayList<>();
                factors.add(71);
                SmallPrimes.boundedTrialDivision(n, bound, factors);
                System.out.println("bounded." + n + "." + bound + "=" + factors);
            }
        }
        for (int n : new int[] {3, 5, 561, 1105, 1729, 2047, 2049, 1373651, 1373653,
                                1373655, 25325999, 25326001, 25326003, 321197185,
                                2147483629, Integer.MAX_VALUE})
            System.out.println("miller." + n + "=" + SmallPrimes.millerRabinPrimeTest(n));
    }

    public static void main(String[] ignored) {
        for (int n = -8; n <= 1024; ++n) publicOperations(n);
        for (int n : new int[] {Integer.MIN_VALUE, -1, 2047, 2048, 2049, 1373652, 1373653,
                                1373654, 25326000, 25326001, 25326002, 3673 * 3677,
                                46337 * 46337, 2147483628, 2147483629, 2147483646, Integer.MAX_VALUE})
            publicOperations(n);
        int state = 123456789;
        for (int i = 0; i < 128; ++i) {
            state = state * 1664525 + 1013904223;
            publicOperations(state & Integer.MAX_VALUE);
        }
        helpers();
        publicOperations(Integer.MAX_VALUE); // Repeated initialized class use.
        System.out.println("oracle=ok");
    }
}
