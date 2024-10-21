import React, { ReactNode } from 'react';

function isMobileDevice(): boolean {
  const isUserAgentMobile = /Mobi|Android|iPhone|iPad|iPod|Opera Mini|IEMobile|WPDesktop/i.test(navigator.userAgent);
  const isScreenSizeMobile = window.matchMedia("(max-width: 768px)").matches;

  return isUserAgentMobile || isScreenSizeMobile;
}

interface DeviceSwitchProps {
  mobile?: ReactNode;
  desktop?: ReactNode;
}

const DeviceSwitch: React.FC<DeviceSwitchProps> = ({ mobile, desktop }) => {
  return isMobileDevice() ? <>{mobile}</> : <>{desktop}</>;
};

export default DeviceSwitch;
