package app.takedock.observer;

import org.junit.Test;
import java.util.List;
import static org.junit.Assert.*;
import static app.takedock.observer.CameraStateReader.*;

public class CameraStateReaderTest {
    private Snapshot read(Node... nodes) { return CameraStateReader.read("net.sourceforge.opencamera", List.of(nodes)); }
    private Node node(String id, String description) { return new Node("net.sourceforge.opencamera:id/" + id, description, true); }

    @Test public void videoIdleIsEligible() {
        Snapshot value = read(node("take_photo", "Start recording video"));
        assertTrue(value.foreground()); assertTrue(value.videoMode()); assertEquals(State.IDLE, value.state());
    }
    @Test public void recordingIsDistinguishedFromPause() {
        assertEquals(State.RECORDING, read(node("take_photo", "Stop recording video"), node("pause_video", "Pause video recording")).state());
        assertEquals(State.PAUSED, read(node("take_photo", "Stop recording video"), node("pause_video", "Resume video recording")).state());
    }
    @Test public void photoModeIsNotEligible() {
        Snapshot value = read(node("take_photo", "Take photo"));
        assertTrue(value.foreground()); assertFalse(value.videoMode()); assertEquals(State.UNKNOWN, value.state());
    }
    @Test public void missingOrUnrecognizedEvidenceIsUnknown() {
        assertEquals(State.UNKNOWN, read().state());
        assertEquals(State.UNKNOWN, read(node("take_photo", "Unrecognized translation")).state());
        assertEquals(State.UNKNOWN, read(node("take_photo", "Stop recording video")).state());
    }
    @Test public void anotherAppCannotProvideCameraEvidence() {
        assertFalse(CameraStateReader.read("com.android.launcher", List.of(node("take_photo", "Start recording video"))).foreground());
    }
    @Test public void invisibleAndUnrelatedNodesAreIgnored() {
        assertEquals(State.UNKNOWN, read(new Node("net.sourceforge.opencamera:id/take_photo", "Start recording video", false)).state());
        assertEquals(State.UNKNOWN, read(new Node("other:id/take_photo", "Start recording video", true)).state());
    }
    @Test public void usesInstalledLocalizedLabelsInsteadOfGuessing() {
        Labels labels = new Labels("Begin", "End", "Hold", "Continue");
        assertEquals(State.PAUSED, CameraStateReader.read("net.sourceforge.opencamera", List.of(node("take_photo", "End"), node("pause_video", "Continue")), labels).state());
    }
}
