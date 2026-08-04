public class Bits {
    public int shift(int a, int n) { return a << n; }
    public int ushift(int a) { return a >>> 8; }
    public int negmod(int a, int b) { return a % b; }
    public char bump(char c) { return (char)(c + 1); }
    public int narrow(int big) { return (byte) big; }
}
