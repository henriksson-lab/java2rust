interface LabelChooser {
    String choose(boolean left);
    String read();
}

public class AnonymousInterfaceNestedStringWrite {
    public static void main(String[] args) {
        LabelChooser chooser = new LabelChooser() {
            String label = "start";

            public String choose(boolean left) {
                if (left) {
                    this.label = "left";
                } else {
                    this.label = "right";
                }
                return this.label;
            }

            public String read() {
                return this.label;
            }
        };
        int first = chooser.choose(true).length();
        int second = chooser.choose(false).length();
        System.out.println(first * 100 + second * 10 + chooser.read().length());
    }
}
