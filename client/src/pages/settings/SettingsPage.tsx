import { useEffect, useState } from "react";
import { useSetUserPrompt, useUserPrompt } from "../../api/hooks";
import { useToasts } from "../../shell/toasts";

export function SettingsPage() {
  const { addToast } = useToasts();
  const userPrompt = useUserPrompt();
  const setUserPrompt = useSetUserPrompt();

  const [content, setContent] = useState("");
  const [loadedContent, setLoadedContent] = useState("");

  // Seed the editable draft once the initial fetch resolves.
  useEffect(() => {
    if (userPrompt.data !== undefined) {
      setContent(userPrompt.data);
      setLoadedContent(userPrompt.data);
    }
  }, [userPrompt.data]);

  const dirty = content !== loadedContent;

  const save = () => {
    setUserPrompt.mutate(content, {
      onSuccess: () => {
        setLoadedContent(content);
        addToast("Prompt saved", "ok");
      },
      onError: (err) => addToast(`Save failed: ${err.message}`, "err"),
    });
  };

  return (
    <div>
      <h1 className="page-title">Settings</h1>
      <div className="card" style={{ padding: 16 }}>
        <div style={{ fontWeight: 700, marginBottom: 4 }}>Persistent prompt</div>
        <p style={{ color: "var(--text-dim)", fontSize: 13, marginTop: 0 }}>
          Appended to every routine&apos;s agent instructions file (CLAUDE.md/AGENTS.md), alongside
          the moadim-managed preamble, on every run.
        </p>
        {userPrompt.isLoading ? (
          <div>Loading…</div>
        ) : (
          <>
            <textarea
              className="form-textarea"
              rows={12}
              placeholder="e.g. always run `cargo fmt` before finishing a task"
              value={content}
              onChange={(e) => setContent(e.target.value)}
            />
            <div style={{ display: "flex", alignItems: "center", gap: 10, marginTop: 10 }}>
              <button
                className="btn btn-primary"
                disabled={!dirty || setUserPrompt.isPending}
                onClick={save}
              >
                {setUserPrompt.isPending ? "Saving…" : "Save"}
              </button>
              {dirty && <span style={{ color: "var(--text-faint)", fontSize: 12 }}>unsaved changes</span>}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
