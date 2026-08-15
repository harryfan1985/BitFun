import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Badge } from '@/component-library';
import { buddyAPI, type BuddyStatusResponse } from '@/infrastructure/api';

interface BuddyStatusPillProps {
  onClick?: () => void;
}

const BuddyStatusPill: React.FC<BuddyStatusPillProps> = ({ onClick }) => {
  const { t } = useTranslation('settings/buddy');
  const [status, setStatus] = useState<BuddyStatusResponse | null>(null);

  const poll = useCallback(async () => {
    try {
      const sts = await buddyAPI.getStatus();
      setStatus(sts);
    } catch {
      // ignore polling errors silently
    }
  }, []);

  useEffect(() => {
    void poll();
    const interval = setInterval(poll, 10000);
    return () => clearInterval(interval);
  }, [poll]);

  if (!status?.enabled) return null;

  const variant = status.bridgeOnline
    ? status.deviceConnected
      ? 'success'
      : 'warning'
    : 'error';

  const label = status.bridgeOnline
    ? status.deviceConnected
      ? status.deviceName ?? 'Buddy'
      : t('pill.scanning')
    : t('pill.offline');

  const title = status.deviceConnected
    ? `${status.deviceName} - ${status.pendingPrompts} pending`
    : t('pill.offline');

  if (onClick) {
    return (
      <span
        role="button"
        tabIndex={0}
        style={{ cursor: 'pointer' }}
        onClick={onClick}
        onKeyDown={(e) => e.key === 'Enter' && onClick()}
        title={title}
      >
        <Badge variant={variant}>{label}</Badge>
      </span>
    );
  }

  return (
    <span title={title}>
      <Badge variant={variant}>{label}</Badge>
    </span>
  );
};

export default BuddyStatusPill;
