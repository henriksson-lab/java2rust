import java.util.Optional;

public class ErasedOptionalDisplay {
    public static String run() {
        Object present = Optional.of("x");
        Object empty = Optional.empty();
        return "p=" + present + ",e=" + empty + ",direct=" + present.toString();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
