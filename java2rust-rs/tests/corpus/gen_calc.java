package com.example;
import java.util.List;
public class Calc {
    private int total = 0;
    public int addAll(int[] xs) {
        for (int i = 0; i < xs.length; i++) {
            total += xs[i];
        }
        if (total > 100) { return 100; } else { return total; }
    }
    static double half(double v) { return v / 2; }
}