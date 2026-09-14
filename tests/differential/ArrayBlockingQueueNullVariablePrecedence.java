import java.util.concurrent.ArrayBlockingQueue;

public class ArrayBlockingQueueNullVariablePrecedence {
    public static String run() throws Exception {
        String addValue = null;
        String offerValue = null;
        String putValue = null;

        ArrayBlockingQueue<String> addQueue = new ArrayBlockingQueue<String>(1);
        addQueue.add("filled");
        String addResult;
        try {
            addQueue.add(addValue);
            addResult = "add-ok";
        } catch (NullPointerException ex) {
            addResult = "add-npe";
        } catch (IllegalStateException ex) {
            addResult = "add-full";
        }

        ArrayBlockingQueue<String> offerQueue = new ArrayBlockingQueue<String>(1);
        offerQueue.add("filled");
        String offerResult;
        try {
            offerResult = "offer-" + offerQueue.offer(offerValue);
        } catch (NullPointerException ex) {
            offerResult = "offer-npe";
        }

        ArrayBlockingQueue<String> putQueue = new ArrayBlockingQueue<String>(1);
        putQueue.add("filled");
        String putResult;
        try {
            putQueue.put(putValue);
            putResult = "put-ok";
        } catch (NullPointerException ex) {
            putResult = "put-npe";
        } catch (InterruptedException ex) {
            putResult = "put-interrupted";
        }

        return addResult + "," + offerResult + "," + putResult;
    }

    public static void main(String[] args) throws Exception {
        System.out.println(run());
    }
}
