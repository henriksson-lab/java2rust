import java.util.Random;

public class RandomNextIntCatch {
    public static String run() {
        Random random = new Random();
        try {
            random.nextInt(0);
            return "miss";
        } catch (IllegalArgumentException ex) {
            return "caught";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
