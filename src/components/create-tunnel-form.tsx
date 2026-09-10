import { useState, type FormEvent } from "react";
import { Loader2, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useCreateTunnel } from "@/hooks/use-tunnels";

export function CreateTunnelForm() {
  const [port, setPort] = useState("");
  const [label, setLabel] = useState("");
  const create = useCreateTunnel();

  const portNumber = Number(port);
  const portValid =
    port !== "" && Number.isInteger(portNumber) && portNumber >= 1 && portNumber <= 65535;

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!portValid || create.isPending) return;
    create.mutate(
      { port: portNumber, label: label.trim() || undefined },
      {
        onSuccess: () => {
          setPort("");
          setLabel("");
        },
      },
    );
  }

  return (
    <form onSubmit={handleSubmit} className="flex items-center gap-2">
      <Input
        type="number"
        inputMode="numeric"
        placeholder="本机端口，如 3000"
        value={port}
        onChange={(e) => setPort(e.target.value)}
        className="selectable w-44"
        min={1}
        max={65535}
        aria-label="本机端口"
      />
      <Input
        placeholder="备注（可选）"
        value={label}
        onChange={(e) => setLabel(e.target.value)}
        className="selectable flex-1"
        aria-label="备注"
      />
      <Button type="submit" disabled={!portValid || create.isPending}>
        {create.isPending ? <Loader2 className="animate-spin" /> : <Plus />}
        {create.isPending ? "建立中" : "映射"}
      </Button>
    </form>
  );
}
