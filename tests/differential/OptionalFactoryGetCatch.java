import java.util.NoSuchElementException;
import java.util.Optional;

public class OptionalFactoryGetCatch {
    public static String run() {
        Optional<String> full = Optional.of("x");
        Optional<String> empty = Optional.empty();
        Optional<String> nullable = Optional.ofNullable(null);
        String status;
        try {
            empty.get();
            status = "miss";
        } catch (NoSuchElementException e) {
            status = "caught";
        }
        return full.get() + ":" + full.isPresent() + ":" + nullable.isPresent() + ":" + status;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
