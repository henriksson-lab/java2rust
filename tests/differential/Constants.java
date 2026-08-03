public class Constants {
    public static String run() {
        int best = Integer.MAX_VALUE;
        for (int x : new int[]{5, 3, 9}) { if (x < best) best = x; }
        int worst = Integer.MIN_VALUE;
        for (int x : new int[]{-5, -3}) { if (x > worst) worst = x; }
        boolean piOk = Math.PI > 3.14 && Math.PI < 3.15;
        boolean minPositive = Double.MIN_VALUE > 0;
        return best + "," + worst + "," + piOk + "," + minPositive;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
