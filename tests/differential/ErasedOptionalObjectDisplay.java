import java.util.Optional;

public class ErasedOptionalObjectDisplay {
    public static String run() {
        Object present = Optional.of(new Object());
        Object empty = Optional.empty();
        return ("p=" + present + ",e=" + empty).replaceAll("@[0-9a-f]+", "@0");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
