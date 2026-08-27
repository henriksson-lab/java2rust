public class StringBuilderCapacityCatch {
    public static String run() {
        StringBuilder builder = new StringBuilder(3);
        builder.append("ok");
        String builderStatus = "";
        try {
            new StringBuilder(-1);
            builderStatus = "miss";
        } catch (NegativeArraySizeException e) {
            builderStatus = "builder";
        }
        String bufferStatus = "";
        try {
            new StringBuffer(-2);
            bufferStatus = "miss";
        } catch (NegativeArraySizeException e) {
            bufferStatus = "buffer";
        }
        return builder.toString() + ":" + builderStatus + ":" + bufferStatus;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
