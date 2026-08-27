import java.util.Optional;

public class OptionalOfNullCatch {
    public static String run() {
        try {
            Optional<String> value = Optional.of(null);
            return value.isPresent() ? "present" : "empty";
        } catch (NullPointerException e) {
            return "npe";
        }
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
