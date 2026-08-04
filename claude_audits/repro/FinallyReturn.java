public class FinallyReturn {
    public int run() {
        try { return 1; }
        finally { System.out.println("cleanup"); }
    }
    public int override() {
        try { return 1; }
        finally { return 2; }
    }
}
