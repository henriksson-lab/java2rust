public class Q3 {
    public double nan() { return 0.0 / 0.0; }
    public int nanCast() { return (int)(0.0/0.0); }
    public int infCast() { return (int)(1.0/0.0); }
    public long minDiv() { return Integer.MIN_VALUE / -1; }
}
