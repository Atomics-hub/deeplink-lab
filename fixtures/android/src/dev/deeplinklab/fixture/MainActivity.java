package dev.deeplinklab.fixture;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Color;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.view.Gravity;
import android.widget.LinearLayout;
import android.widget.TextView;

import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;

public final class MainActivity extends Activity {
    private static final String TAG = "DeepLinkFixture";
    private TextView destinationView;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        buildUi();
        record("home");
        handle(getIntent(), false);
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handle(intent, true);
    }

    private void buildUi() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        layout.setGravity(Gravity.CENTER);
        layout.setPadding(40, 40, 40, 40);
        layout.setBackgroundColor(Color.rgb(9, 19, 36));

        TextView title = new TextView(this);
        title.setText("DeepLink Lab Fixture");
        title.setTextColor(Color.WHITE);
        title.setTextSize(28);
        title.setGravity(Gravity.CENTER);
        layout.addView(title);

        destinationView = new TextView(this);
        destinationView.setTextColor(Color.rgb(104, 177, 255));
        destinationView.setTextSize(20);
        destinationView.setGravity(Gravity.CENTER);
        destinationView.setPadding(0, 48, 0, 48);
        destinationView.setContentDescription("deeplinklab.destination");
        layout.addView(destinationView);

        TextView note = new TextView(this);
        note.setText("This app intentionally contains deterministic routing faults.");
        note.setTextColor(Color.LTGRAY);
        note.setTextSize(14);
        note.setGravity(Gravity.CENTER);
        layout.addView(note);
        setContentView(layout);
    }

    private void handle(Intent intent, boolean continuation) {
        Uri uri = intent == null ? null : intent.getData();
        if (uri == null) return;
        String family = uri.getHost() == null ? "" : uri.getHost();
        String path = trim(uri.getPath());
        String destination;
        switch (family) {
            case "wrong":
                destination = "home";
                break;
            case "query-loss":
                destination = "search";
                break;
            case "fragment-loss":
                destination = path;
                break;
            case "route-collision":
                destination = "user/admin";
                break;
            case "warm-stale":
            case "background-ignore":
                if (continuation) return;
                destination = path;
                break;
            case "encoding-collapse":
                destination = trim(uri.getPath());
                break;
            default:
                destination = canonical(uri, path);
        }
        record(destination);
    }

    private String canonical(Uri uri, String path) {
        StringBuilder value = new StringBuilder(path);
        String query = uri.getQueryParameter("q");
        if (query != null) value.append("?q=").append(query);
        if (uri.getFragment() != null) value.append("#").append(uri.getFragment());
        return value.toString();
    }

    private String trim(String value) {
        if (value == null) return "";
        return value.startsWith("/") ? value.substring(1) : value;
    }

    private void record(String destination) {
        destinationView.setText("Destination: " + destination);
        destinationView.setContentDescription("Destination: " + destination);
        Log.i(TAG, "DEEPLINKLAB_DESTINATION=" + destination);
        File file = new File(getFilesDir(), "destination.txt");
        try (FileOutputStream output = new FileOutputStream(file, false)) {
            output.write(destination.getBytes(StandardCharsets.UTF_8));
        } catch (Exception error) {
            Log.e(TAG, "destination evidence write failed", error);
        }
    }
}
